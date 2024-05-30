#! /bin/bash

# upload to gns3 server
# $1 is the ip address of the gns3 server
# after uploading, must run `sudo ./sync.sh`

cargo build
sshpass -p gns3 scp target/debug/ospfd gns3@$1:ospfd/ospfd-debug

cargo build -r
sshpass -p gns3 scp target/release/ospfd gns3@$1:ospfd/ospfd
