#!/bin/bash
basen=$(basename $1)
name="${basen%.*}"
aseprite -b $1 --sheet ./sheets/$name.png --data ./sheets/$name.json --format json-array
