#!/bin/sh -eu

cargo build
cc -Wall -Wextra -Og test.c target/debug/liblibnyx.a -o app
exec ./app
