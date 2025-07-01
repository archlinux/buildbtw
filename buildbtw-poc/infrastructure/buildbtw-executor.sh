#!/usr/bin/bash
set -o nounset -o errexit -o pipefail -o xtrace

# https://docs.gitlab.com/runner/executors/custom/#config
config() {
    # Set a custom build directory for cloning package sources.
    # Since we're cloning sources outside of VMs
    # until https://github.com/svenstaro/vmexec/issues/12 is fixed,
    # we need to make sure build dirs don't conflict with each other
    # TODO investigate whether we should set "builds_dir_is_shared" to false
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
    # the host should be reachable at 10.0.2.2 since we're using
    # user mode networking
    pacman_repo_url="http://10.0.2.2:8080/repo/${CUSTOM_ENV_NAMESPACE_NAME}_${CUSTOM_ENV_ITERATION_ID}/os/${CUSTOM_ENV_ARCHITECTURE}"
    output_dir=$(sudo -u buildbtw mktemp -d)

    sudo -u buildbtw --set-home \
    vmexec run archlinux --rm --pmem /var/lib/archbuild:30 \
        --ssh-timeout 120 \
        --volume "${CUSTOM_ENV_CI_PROJECT_DIR}":/mnt/src_repo:ro \
        --volume /srv/buildbtw/gitlab-executor:/mnt/bin:ro \
        --volume "${output_dir}":/mnt/output \
        -- \
        /mnt/bin/build-inside-vm.sh "${pacman_repo_url}" || exit "${BUILD_FAILURE_EXIT_CODE:-1}"

    tree "$output_dir"
    package_file_names=(${CUSTOM_ENV_PACKAGE_FILE_NAMES})
    for file in "${package_file_names[@]}"; do
        # extract everything before first hyphen followed by digit
        pkgname="${file%%-[0-9]*}"

        sudo -u buildbtw --set-home curl -v -X POST --data-binary @"${output_dir}/${file}" "http://127.0.0.1:8080/iteration/${CUSTOM_ENV_ITERATION_ID}/pkgbase/${CUSTOM_ENV_PKGBASE}/pkgname/${pkgname}/architecture/${CUSTOM_ENV_ARCHITECTURE}/package"
    done

    rm -rf "${output_dir}"
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
