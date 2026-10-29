#!/bin/bash
PIDFILE="/tmp/hyprsunset.pid"

if [ -f "$PIDFILE" ]; then
    kill $(cat $PIDFILE)
    rm $PIDFILE
    pkill hyprsunset
    pkill -RTMIN+1 waybar
else
    hyprsunset -t 3500 &
    echo $! > $PIDFILE
    pkill -RTMIN+1 waybar
fi
