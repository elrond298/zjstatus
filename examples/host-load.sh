#!/bin/sh

samples=10
history_samples=$((samples - 1))
stale_after=120
load_scale=${ZJSTATUS_LOAD_SCALE:-1}
net_scale=${ZJSTATUS_NET_SCALE_BPS:-125000000}
disk_scale=${ZJSTATUS_DISK_SCALE_BPS:-1000000000}
for value in "$load_scale" "$net_scale" "$disk_scale"; do
    case $value in ''|*[!0-9]*|0) exit 0 ;; esac
done
load_scale=$((load_scale * 100))

runtime=${XDG_RUNTIME_DIR:-/tmp}/zjstatus-metrics-$(id -u)
session=session-$(printf '%s' "${ZELLIJ_SESSION_NAME:-default}" | sed 's/[^A-Za-z0-9_.-]/_/g')
state=$runtime/$session
panel=\$panel
mkdir -p "$runtime" 2>/dev/null || exit 0
chmod 700 "$runtime" 2>/dev/null || exit 0

now=$(date +%s)
bucket=$((now / 60))
load=$(awk '{print $1; exit}' /proc/loadavg) || exit 0
load_sample=$(awk -v value="$load" 'BEGIN { printf "%.0f", value * 100 }')
net=$(awk -F: '$1 !~ /lo$/ {
    gsub(/^ +/, "", $2); split($2, value, / +/); total += value[1] + value[9]
} END { printf "%.0f", total }' /proc/net/dev)

disk=0
for stats in /sys/block/*/stat; do
    device=${stats%/stat}
    device=${device##*/}
    case $device in loop*|ram*|zram*|dm-*|md*) continue ;; esac
    read -r _ _ read_sectors _ _ _ write_sectors _ <"$stats"
    disk=$((disk + read_sectors + write_sectors))
done

old_time='' old_net='' old_disk='' old_bucket=''
load_peak=0 net_peak=0 disk_peak=0
load_history='' net_history='' disk_history=''
if [ -r "$state" ]; then
    {
        read -r old_time old_net old_disk old_bucket
        read -r load_peak net_peak disk_peak
        read -r load_history
        read -r net_history
        read -r disk_history
    } <"$state"
fi
case $old_bucket in ''|*[!0-9]*) old_bucket=''; load_history=''; net_history=''; disk_history='' ;; esac

elapsed=$((now - ${old_time:-now}))
net_rate=0
disk_rate=0
if [ "$elapsed" -gt 0 ] && [ "$elapsed" -le "$stale_after" ]; then
    if [ "$net" -ge "${old_net:-$net}" ]; then
        net_rate=$(((net - old_net) / elapsed))
    else
        net_history=''; net_peak=0
    fi
    if [ "$disk" -ge "${old_disk:-$disk}" ]; then
        disk_rate=$(((disk - old_disk) * 512 / elapsed))
    else
        disk_history=''; disk_peak=0
    fi
fi

append() {
    printf '%s %s\n' "$1" "$2" | awk -v keep="$history_samples" '{
        first = NF - keep + 1; if (first < 1) first = 1
        for (i = first; i <= NF; i++) printf "%s%s", (i == first ? "" : " "), $i
    }'
}

if [ -z "$old_bucket" ] || [ "$bucket" -lt "$old_bucket" ]; then
    load_history=''; net_history=''; disk_history=''
    load_peak=$load_sample; net_peak=$net_rate; disk_peak=$disk_rate
elif [ "$bucket" -gt "$old_bucket" ]; then
    gap=$((bucket - old_bucket))
    if [ "$gap" -ge "$samples" ]; then
        load_history=''; net_history=''; disk_history=''
    else
        load_history=$(append "$load_history" "$load_peak")
        net_history=$(append "$net_history" "$net_peak")
        disk_history=$(append "$disk_history" "$disk_peak")
        missing=$((gap - 1))
        while [ "$missing" -gt 0 ]; do
            load_history=$(append "$load_history" 0)
            net_history=$(append "$net_history" 0)
            disk_history=$(append "$disk_history" 0)
            missing=$((missing - 1))
        done
    fi
    load_peak=$load_sample; net_peak=$net_rate; disk_peak=$disk_rate
else
    [ "$load_sample" -le "$load_peak" ] || load_peak=$load_sample
    [ "$net_rate" -le "$net_peak" ] || net_peak=$net_rate
    [ "$disk_rate" -le "$disk_peak" ] || disk_peak=$disk_rate
fi

load_graph="$load_history $load_peak"
net_graph="$net_history $net_peak"
disk_graph="$disk_history $disk_peak"
tmp=$state.$$
{
    printf '%s %s %s %s\n' "$now" "$net" "$disk" "$bucket"
    printf '%s %s %s\n' "$load_peak" "$net_peak" "$disk_peak"
    printf '%s\n%s\n%s\n' "$load_history" "$net_history" "$disk_history"
} >"$tmp" && mv -f "$tmp" "$state"
trap 'rm -f "$tmp"' EXIT HUP INT TERM

braille() {
    printf '%s\n' "$1" | LC_ALL=C awk -v count="$samples" -v scale="$2" '
        function height(value) {
            if (value <= 0) return 0
            if (value >= scale) return 4
            value = int(value * 4 / scale)
            return value < 1 ? 1 : value
        }
        function utf8(code) {
            return sprintf("%c%c%c", 224 + int(code / 4096), 128 + int(code / 64) % 64, 128 + code % 64)
        }
        {
            left[0] = 0; left[1] = 64; left[2] = 68; left[3] = 70; left[4] = 71
            right[0] = 0; right[1] = 128; right[2] = 160; right[3] = 176; right[4] = 184
            padding = count - NF
            for (i = 1; i <= padding; i++) values[i] = 0
            for (i = 1; i <= NF; i++) values[padding + i] = $i
            for (i = 1; i <= count; i += 2)
                printf "%s", utf8(10240 + left[height(values[i])] + right[height(values[i + 1])])
        }
    '
}

rate() {
    awk -v bytes="$1" 'BEGIN {
        if (bytes >= 1099511627776) { value = bytes / 1099511627776; unit = "T" }
        else if (bytes >= 1073741824) { value = bytes / 1073741824; unit = "G" }
        else if (bytes >= 1048576) { value = bytes / 1048576; unit = "M" }
        else if (bytes >= 1024) { value = bytes / 1024; unit = "K" }
        else { value = bytes; unit = "B" }
        if (value > 999.9) value = 999.9
        printf "%5.1f%s", value, unit
    }'
}

block() {
    printf "#[bg=$panel,fg=%s]%s#[bg=$panel] " "$1" "$2"
}

load_label=$(awk -v value="$load" 'BEGIN { if (value > 999.9) value = 999.9; printf "%5.1f", value }')
block '#C6A0F6' "󰋊 $(rate "$disk_rate")/s $(braille "$disk_graph" "$disk_scale")"
block '#8AADF4' "󰖩 $(rate "$net_rate")/s $(braille "$net_graph" "$net_scale")"
block '#EED49F' " $load_label"
block '#EED49F' "$(braille "$load_graph" "$load_scale")"
