#!/bin/bash

openssl req -x509 -newkey rsa:4096 -nodes -keyout ca.key -out ca.crt -days 365 -subj '/CN=test CA/emailAddress=mnow@si-int.eu' -addext 'subjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30'

openssl req -newkey rsa:4096 -nodes -keyout server.key -out server.req -subj "/CN=test server/emailAddress=mnow@si-int.eu"
openssl x509 -req -in server.req -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 365 -extfile <(printf '\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')

openssl req -newkey rsa:4096 -nodes -keyout client.key -out client.req -subj "/CN=test client/emailAddress=mnow@si-int.eu"
openssl x509 -req -in client.req -CA ca.crt -CAkey ca.key -CAcreateserial -out client.crt -days 365 -extfile <(printf '\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\n')

openssl req -x509 -newkey rsa:4096 -nodes -keyout evil-server.key -out evil-server.crt -subj "/CN=test evil server/emailAddress=mnow@si-int.eu" -addext 'subjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30' -addext 'basicConstraints=CA:FALSE' -addext 'keyUsage=nonRepudiation,digitalSignature,keyEncipherment' -addext 'extendedKeyUsage=serverAuth'

openssl req -x509 -newkey rsa:4096 -nodes -keyout evil-client.key -out evil-client.crt -subj "/CN=test evil client/emailAddress=mnow@si-int.eu" -addext 'subjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30' -addext 'basicConstraints=CA:FALSE' -addext 'keyUsage=nonRepudiation,digitalSignature,keyEncipherment' -addext 'extendedKeyUsage=clientAuth'

openssl req -newkey rsa:4096 -nodes -keyout broker.key -out broker.req -subj "/CN=test broker/emailAddress=mnow@si-int.eu"
openssl x509 -req -in broker.req -CA ca.crt -CAkey ca.key -CAcreateserial -out broker.crt -days 365 -extfile <(printf '\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')

openssl req -newkey rsa:4096 -nodes -keyout backend.key -out backend.req -subj "/CN=test backend/O=si/emailAddress=mnow@si-int.eu"
openssl x509 -req -in backend.req -CA ca.crt -CAkey ca.key -CAcreateserial -out backend.crt -days 365 -extfile <(printf '\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\n')

openssl req -newkey rsa:4096 -nodes -keyout agent1.key -out agent1.req -subj "/CN=test agent1/O=si/emailAddress=mnow@si-int.eu"
openssl x509 -req -in agent1.req -CA ca.crt -CAkey ca.key -CAcreateserial -out agent1.crt -days 365 -extfile <(printf '\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\n')

openssl req -newkey rsa:4096 -nodes -keyout agent2.key -out agent2.req -subj "/CN=test agent2/O=si/emailAddress=mnow@si-int.eu"
openssl x509 -req -in agent2.req -CA ca.crt -CAkey ca.key -CAcreateserial -out agent2.crt -days 365 -extfile <(printf '\n[default]\nsubjectAltName=DNS:mndev02,DNS:mndev02.sit.be,DNS:localhost,IP:127.0.0.1,IP:192.168.10.30\nbasicConstraints=CA:FALSE\nkeyUsage=nonRepudiation,digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\n')

rm -f *.req
