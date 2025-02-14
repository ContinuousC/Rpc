#!/bin/bash

if [ -e ca ]; then
    echo "Error: ca certificate already exists; please remove it first!" >&2
    exit 1
fi

mkdir -p ca
chmod u=rwx,og= ca

openssl req -x509 -newkey rsa:4096 -nodes -keyout ca/ca.key -out ca/ca.crt -days 3650 -subj '/CN=ContinuousC test CA/emailAddress=mnow@si-int.eu' -addext 'subjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30'
