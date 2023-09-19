#!/bin/bash
./copy-from-external.sh
find ./src/*.aseprite | xargs -r -t -n 1 ./game-ready.sh
