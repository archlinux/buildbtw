#!/usr/bin/env bash

set -euxo pipefail

password(){
  read -ersp "Enter a password for testuser: " PASSWORD
}

echo "Pulling Authelia docker image for setup"
docker pull docker.io/authelia/authelia > /dev/null

DOMAIN="buildbtw.localhost"

# TODO move this into the justfile and run it automatically
echo "Generating SSL certificate for *.$DOMAIN"
mkcert -cert-file authelia/certificate.pem -key-file authelia/key.pem "*.$DOMAIN"

password

if [[ $PASSWORD != "" ]]; then
  PASSWORD=$(docker run authelia/authelia authelia crypto hash generate argon2 --password "$PASSWORD" | sed 's/Digest: //g')
  sed -i "s/<PASSWORD>/$(echo "$PASSWORD" | sed -e 's/[\/&]/\\&/g')/g" authelia/users_database.yml
else
  echo "Password cannot be empty"
  password
fi

docker compose up -d

cat << EOF
Setup completed successfully.

You can now visit the following locations:
- https://authelia.$DOMAIN - Access Authelia directly
- https://secure.$DOMAIN - Secured with Authelia one-factor authentication

You will need to authorize the self-signed certificate upon visiting each domain.
EOF
