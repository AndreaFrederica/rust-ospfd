#! /bin/bash

for name in $(docker volume ls | sed 1d | awk '{print $2}')
do
cp ospf* /opt/docker/volumes/$name/_data
done
