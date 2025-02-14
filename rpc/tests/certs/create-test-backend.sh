#!/bin/bash

org=$1

if [ -z $org ]; then
    echo "Usage: $0 <name-of-organization>" >&2
    exit 1
fi

if ! [ -e ca/ca.crt ]; then
    echo "Please create the test CA first by running: bash create-test-ca.sh" >&2
    exit 1
fi

if [ -e "$org" ]; then
    echo "Error: $org certificates already exist; please remove them first!" >&2
    exit 1
fi

mkdir "$org"
chmod u=rwx,og= $org


# Intermediate CA

openssl req -newkey rsa:4096 -keyout "$org/backend-ca.key" -nodes -out "$org/backend-ca.req" -subj "/O=$org/CN=ContinuousC Intermediate CA/emailAddress=mnow@si-int.eu"
openssl x509 -req -in "$org/backend-ca.req" -CA ca/ca.crt -CAkey ca/ca.key -CAcreateserial -out "$org/backend-ca.crt" -days 1095 -extfile <(printf '\n[default]\nbasicConstraints=critical,CA:TRUE\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid:always,issuer\nkeyUsage=cRLSign,keyCertSign')

rm -f "$org/backend-ca.req"
cp -a ca/ca.crt "$org/"

function create_cert {

    name=$1
    subj=$2
    usage=$3
    
    openssl req -newkey rsa:4096 -keyout "$org/$name.key" -nodes -out "$org/$name.req" -subj "$subj"
    openssl x509 -req -in "$org/$name.req" -CA "$org/backend-ca.crt" -CAkey "$org/backend-ca.key" -CAcreateserial -out "$org/$name.crt" -days 1095 -extfile <(printf "\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=$usage\n")

    cat "$org/backend-ca.crt" >> "$org/$name.crt"
    rm -f "$org/$name.req"

}

create_cert server "/CN=mndev02/emailAddress=mnow@si-int.eu" serverAuth
create_cert backend "/O=$org/CN=ContinuousC backend/emailAddress=mnow@si-int.eu" clientAuth
create_cert dbdaemon "/O=$org/CN=ContinuousC Database Daemon/emailAddress=mnow@si-int.eu" serverAuth
create_cert metrics-engine-server "/O=$org/CN=ContinuousC Metrics Engine/emailAddress=mnow@si-int.eu" serverAuth
create_cert metrics-engine-client "/O=$org/CN=ContinuousC Metrics Engine/emailAddress=mnow@si-int.eu" clientAuth
