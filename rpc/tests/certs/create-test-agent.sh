#!/bin/bash

org=$1
agent=$2

if [ -z $org -o -z $agent ]; then
    echo "Usage: $0 <name-of-organization> <id-of-agent>" >&2
    exit 1
fi

if ! [ -e ca/ca.crt ]; then
    echo "Please create the test CA first by running: bash create-test-ca.sh" >&2
    exit 1
fi

if ! [ -e "$org/backend-ca.crt" ]; then
    echo "Please create the backend certs first by running: bash create-test-backend.sh $org" >&2
    exit 1
fi

if [ -e "$org/agent-$agent.crt" ]; then
    echo "Error: $org/agent-$agent certificate already exists; please remove it first!" >&2
    exit 1
fi

openssl req -newkey rsa:4096 -keyout "$org/agent-$agent.key" -nodes -out "$org/agent-$agent.req" -subj "/O=$org/CN=$agent/emailAddress=mnow@si-int.eu"
openssl x509 -req -in "$org/agent-$agent.req" -CA "$org/backend-ca.crt" -CAkey "$org/backend-ca.key" -CAcreateserial -out "$org/agent-$agent.crt" -days 1095 -extfile <(printf "\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\n")

cat "$org/backend-ca.crt" >> "$org/agent-$agent.crt"
rm -f "$org/agent-$agent.req"
