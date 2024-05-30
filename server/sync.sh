#! /bin/bash

cp ospf* /opt/docker/volumes/$(docker volume ls | sed -n 2,2p | awk '{print $2}')/_data
