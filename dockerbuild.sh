#!/bin/sh


sudo docker run -it -v ~/.cargo:/root/.cargo -v $(pwd):/root/src rust:slim "bash"