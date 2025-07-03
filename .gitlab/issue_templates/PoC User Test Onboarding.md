I'd like to test the buildbtw proof-of-concept.

# Availability for onboarding call

- [ ] I'd like to join a voice call for onboarding (please suggest timeframes that suit you).
- [ ] I'll onboard on my own.

# Onboarding checklist for buildbtw team

- [ ] Arrange time and date for voice call
- [ ] Create user on `buildbtw-dev`: `sudo useradd -m ${USER}`
- [ ] Add SSH public key
    ```
    mkdir -p /home/${USER}/.ssh
    echo "${PUBKEY}" > /home/${USER}/.ssh/authorized_keys
    ```
- [ ] Add to [packaging-buildbtw-dev group](https://gitlab.archlinux.org/groups/packaging-buildbtw-dev/-/group_members) with access level 'Developer'
- [ ] In the onboarding call, walk through the [user guide](https://gitlab.archlinux.org/archlinux/buildbtw/-/blob/main/notes/PoC_User_Guide.md)
- [ ] Invite them to the buildbtw IRC/[Matrix](https://matrix.to/#/#buildbtw:archlinux.org) channel
