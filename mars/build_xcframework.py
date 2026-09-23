#!/usr/bin/env python3
"""Build MarsXlog.xcframework for the Swift Package Manager distribution.

The XCFramework is a *binary target* of Package.swift, see ../Package.swift.
Each Apple platform variant is built separately and packaged into its own
slice, because a fat library can not hold both a device arm64 slice and a
simulator arm64 slice.

There is no CMake and no C++ left in the pipeline: every slice is
`libmars_xlog_ffi.a`, the static library of the Rust crate
`rust/crates/mars-xlog-ffi`, cross-compiled by cargo. The public C ABI of
that crate (../rust/crates/mars-xlog-ffi/include/mars_xlog.h) is the umbrella
header of the framework, and Sources/MarsXlog wraps it in the Swift API
Package.swift publishes.

Usage:
    python3 build_xcframework.py [tag]

Output:
    cmake_build/XCFramework/MarsXlog.xcframework
"""

import argparse
import os
import shutil
import subprocess
import sys

from mars_utils import *  # noqa: F401,F403  (lipo_libs / copy_file / remove_if_exist)

SCRIPT_PATH = os.path.split(os.path.realpath(__file__))[0]
RUST_PATH = os.path.join(os.path.dirname(SCRIPT_PATH), 'rust')

BUILD_OUT_PATH = 'cmake_build/XCFramework'
SLICES_PATH = BUILD_OUT_PATH + '/slices'
DIST_PATH = BUILD_OUT_PATH
XCFRAMEWORK_NAME = 'MarsXlog'
# Clang module of the C ABI. It is deliberately *not* `MarsXlog`: that name
# belongs to the Swift target of Package.swift, and a Swift module can not
# import a Clang module of its own name.
MODULE_NAME = 'MarsXlogC'
DEPLOYMENT_TARGET = '12.0'

FFI_CRATE = 'mars-xlog-ffi'
FFI_LIB = 'libmars_xlog_ffi.a'
FFI_HEADER = os.path.join(RUST_PATH, 'crates', FFI_CRATE, 'include', 'mars_xlog.h')

# `PLATFORM` of the old ios.toolchain.cmake -> Rust target triple.
RUST_TARGETS = {
    'OS64': 'aarch64-apple-ios',
    'SIMULATOR64': 'x86_64-apple-ios',
    'SIMULATORARM64': 'aarch64-apple-ios-sim',
}

MODULEMAP = '''module MarsXlogC {
    header "mars_xlog.h"
    export *
}
'''

INFO_PLIST = '''<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
\t<key>CFBundleDevelopmentRegion</key>
\t<string>en</string>
\t<key>CFBundleExecutable</key>
\t<string>{name}</string>
\t<key>CFBundleIdentifier</key>
\t<string>{bundle_id}</string>
\t<key>CFBundleInfoDictionaryVersion</key>
\t<string>6.0</string>
\t<key>CFBundleName</key>
\t<string>{name}</string>
\t<key>CFBundlePackageType</key>
\t<string>FMWK</string>
\t<key>CFBundleShortVersionString</key>
\t<string>1.0</string>
\t<key>CFBundleSupportedPlatforms</key>
\t<array>
\t\t<string>{platform}</string>
\t</array>
\t<key>CFBundleVersion</key>
\t<string>1</string>
\t<key>MinimumOSVersion</key>
\t<string>{min_os}</string>
</dict>
</plist>
'''


def build_slice(platform: str) -> str:
    """Cross-compile one slice with cargo, return the static library path."""
    target = RUST_TARGETS[platform]
    env = os.environ.copy()
    env['IPHONEOS_DEPLOYMENT_TARGET'] = DEPLOYMENT_TARGET
    cmd = ['cargo', 'build', '--locked', '--release',
           '--package', FFI_CRATE, '--target', target]
    print('==== building %s (%s) ====\n%s' % (platform, target, ' '.join(cmd)))
    if subprocess.call(cmd, cwd=RUST_PATH, env=env) != 0:
        raise RuntimeError('cargo build %s fail' % target)

    src = os.path.join(RUST_PATH, 'target', target, 'release', FFI_LIB)
    if not os.path.isfile(src):
        raise RuntimeError('missing static library: %s' % src)

    slices_dir = os.path.join(SCRIPT_PATH, SLICES_PATH)
    os.makedirs(slices_dir, exist_ok=True)
    dst = os.path.join(slices_dir, '%s.a' % platform)
    shutil.copyfile(src, dst)
    return dst


def make_headers() -> str:
    """Stage `mars_xlog.h` next to a module map, the shape `-headers` expects."""
    headers = os.path.join(SCRIPT_PATH, SLICES_PATH, 'headers')
    if os.path.exists(headers):
        shutil.rmtree(headers)
    os.makedirs(headers)
    shutil.copy(FFI_HEADER, os.path.join(headers, os.path.basename(FFI_HEADER)))
    with open(os.path.join(headers, 'module.modulemap'), 'w') as f:
        f.write(MODULEMAP)
    return headers


def build_xcframework(tag: str = '') -> str:
    del tag  # the C++ build stamped it into comm/verinfo.h; Rust needs no revision file

    slices_dir = os.path.join(SCRIPT_PATH, SLICES_PATH)
    if os.path.exists(slices_dir):
        shutil.rmtree(slices_dir)
    os.makedirs(slices_dir)

    # device
    device_lib = build_slice('OS64')
    # simulator: x86_64 (Intel Mac) + arm64 (Apple Silicon), merged into one fat lib
    sim_x86_64_lib = build_slice('SIMULATOR64')
    sim_arm64_lib = build_slice('SIMULATORARM64')
    sim_lib = os.path.join(slices_dir, 'ios-simulator.a')
    if not lipo_libs([sim_x86_64_lib, sim_arm64_lib], sim_lib):  # noqa: F405
        raise RuntimeError('lipo simulator slices fail')

    headers = make_headers()
    dst = os.path.join(SCRIPT_PATH, DIST_PATH, '%s.xcframework' % XCFRAMEWORK_NAME)
    remove_if_exist(dst)  # noqa: F405
    cmd = ('xcodebuild -create-xcframework '
           '-library "%s" -headers "%s" '
           '-library "%s" -headers "%s" -output "%s"'
           % (device_lib, headers, sim_lib, headers, dst))
    print(cmd)
    if subprocess.call(cmd, shell=True) != 0:
        raise RuntimeError('create-xcframework fail')

    print('==================Output========================')
    print(dst)
    return dst


def main() -> int:
    parser = argparse.ArgumentParser(description='Build MarsXlog.xcframework')
    parser.add_argument('tag', nargs='?', default='',
                        help='kept for backwards compatibility, unused')
    parser.add_argument('--zip',
                        action='store_true',
                        help='zip the xcframework, ready to be attached to a GitHub release')
    args = parser.parse_args()

    xcframework = build_xcframework(args.tag)

    if args.zip:
        zip_path = os.path.join(SCRIPT_PATH, DIST_PATH, '%s.xcframework.zip' % XCFRAMEWORK_NAME)
        remove_if_exist(zip_path)  # noqa: F405
        subprocess.check_call(['ditto', '-c', '-k', '--keepParent', xcframework, zip_path])
        print(zip_path)
        if shutil.which('swift') or os.path.isfile('/usr/bin/xcrun'):
            try:
                out = subprocess.check_output(
                    ['swift', 'package', 'compute-checksum', zip_path], stderr=subprocess.STDOUT)
                print('checksum: %s' % out.decode().strip())
            except (OSError, subprocess.CalledProcessError):
                print('run `swift package compute-checksum %s` to get the checksum' % zip_path)
    return 0


if __name__ == '__main__':
    sys.exit(main())
