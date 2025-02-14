#!/bin/bash

if ! [ -e ca/ca.crt ]; then
    echo "Please create the test CA first by running: bash create-test-ca.sh" >&2
    exit 1
fi

if [ -e broker ]; then
    echo "Error: broker certificate already exists; please remove it first!" >&2
    exit 1
fi

mkdir -p broker
chmod u=rwx,og= broker

openssl req -newkey rsa:4096 -nodes -keyout broker/broker.key -out broker/broker.req -subj "/CN=ContinuousC test Broker/emailAddress=mnow@si-int.eu"
openssl x509 -req -in broker/broker.req -CA ca/ca.crt -CAkey ca/ca.key -CAcreateserial -out broker/broker.crt -days 1095 -extfile <(printf '\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')

rm -f broker/broker.req
cp -a ca/ca.crt broker/
