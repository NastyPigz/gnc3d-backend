#!/bin/sh
export $(cat .env | xargs)

cargo run