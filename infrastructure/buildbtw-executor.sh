#!/usr/bin/bash
set -o nounset -o errexit -o pipefail -o xtrace

# https://docs.gitlab.com/runner/executors/custom/#config
config() {
    # Set a custom build directory for cloning package sources.
    # Since we're cloning sources outside of VMs
    # until https://github.com/svenstaro/vmexec/issues/12 is fixed,
    # we need to make sure build dirs don't conflict with each other
    cat << EOS
        {
          "builds_dir": "/srv/buildbtw/gitlab-builds/${CUSTOM_ENV_CI_CONCURRENT_PROJECT_ID}/${CUSTOM_ENV_CI_PROJECT_PATH_SLUG}",
          "cache_dir": "/srv/buildbtw/gitlab-cache/${CUSTOM_ENV_CI_CONCURRENT_PROJECT_ID}/${CUSTOM_ENV_CI_PROJECT_PATH_SLUG}"
        }
EOS
}

# https://docs.gitlab.com/runner/executors/custom.html#prepare
prepare() {
    # Pull image if it doesn't exist and make sure a booted snapshot is available.
    # temporarily disabled for easier debugging
    # vmexec run archlinux --pull newer --pmem /var/lib/archbuild:30 -- echo 'VM started'
    :
}

# https://docs.gitlab.com/runner/executors/custom.html#run
run() {
    # TODO pass repo URL as second argument to build-inside-vm.sh
    # the host should be reachable at 10.0.2.2 since we're using
    # user mode networking
    pacman_repo_url="http://10.0.2.2:8080/repo/${CUSTOM_ENV_NAMESPACE_NAME}_${CUSTOM_ENV_ITERATION_ID}/os/${CUSTOM_ENV_ARCHITECTURE}"
    output_dir="$CUSTOM_ENV_CI_PROJECT_DIR"
    sudo -u buildbtw --set-home \
    vmexec run archlinux --pmem /var/lib/archbuild:30 \
    --volume "$CUSTOM_ENV_CI_PROJECT_DIR":/mnt/src_repo \
    --volume /srv/buildbtw/gitlab-executor:/mnt/bin \
    -- \
        /mnt/bin/build-inside-vm.sh "${pacman_repo_url}" || exit "${BUILD_FAILURE_EXIT_CODE:-1}"

    tree "$output_dir"
    # TODO upload build artifacts
}

# https://docs.gitlab.com/runner/executors/custom.html#cleanup
cleanup() {
    # vmexec mostly cleans up after itself
    # TODO: clean up old versions of VM base images
    :
}

case "${1:-}" in
    config)
        config
        ;;
    prepare)
        prepare
        ;;
    run)
        if [[ ${3} == get_sources ]]; then
            cd /srv/buildbtw/gitlab-builds
            cat "${2}" | sudo -u buildbtw bash
        elif [[ ${3} == build_script ]]; then
            run
        fi
        ;;
    cleanup)
        cleanup
        ;;
    *)
        echo "Error invalid command: ${1:-}"
        exit 1;
esac
