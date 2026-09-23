#!/usr/bin/env python3
"""Build MarsXlog.xcframework for the Swift Package Manager distribution.

The XCFramework is a *binary target* of Package.swift, see ../Package.swift.
Every Apple platform variant is built separately and packaged into its own
slice, because a fat library can not hold both a device arm64 slice and a
simulator arm64 slice.

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

from mars_utils import *  # noqa: F401,F403  (libtool_libs / lipo_libs / clean / gen_mars_revision_file)

SCRIPT_PATH = os.path.split(os.path.realpath(__file__))[0]

BUILD_OUT_PATH = 'cmake_build/XCFramework'
SLICES_PATH = BUILD_OUT_PATH + '/slices'
DIST_PATH = BUILD_OUT_PATH
FRAMEWORK_NAME = 'MarsXlog'
BUNDLE_ID = 'com.tencent.mars.xlog'

# Keep in sync with the platforms declared in ../Package.swift.
IOS_MIN = '12.0'
WATCHOS_MIN = '9.0'

XLOG_STATIC_LIBS = ['libcomm.a', 'libmars-boost.a', 'libxlog.a']
OBJC_HEADER = 'xlog/objc/MarsXlog.h'

# One entry per slice of the xcframework:
#
#   name     -> directory the slice is built into; xcodebuild renames it in
#               the final .xcframework, sorting the architectures
#               (watchos-arm64_32_arm64 comes out as watchos-arm64_arm64_32)
#   platform -> CFBundleSupportedPlatforms written into the wrapper framework,
#               which is how xcodebuild tells the slices apart
#   min_os   -> deployment target of the slice, also its MinimumOSVersion and
#               the version declared in Package.swift
#   variants -> (PLATFORM, ARCHS, build directory) triples for
#               ios.toolchain.cmake; a slice with more than one variant is
#               merged into one fat library with lipo
#
# watchOS is arm64_32 + arm64 on device and arm64 + x86_64 on the simulator.
# 9.0 is the floor: armv7k (the 32-bit watches) retired with watchOS 9, and a
# lower deployment target makes Xcode ask for an armv7k slice the framework
# can not ship.
SLICES = [
    ('ios-arm64', 'iPhoneOS', IOS_MIN, [
        ('OS64', 'arm64', 'ios-arm64'),
    ]),
    ('ios-arm64_x86_64-simulator', 'iPhoneSimulator', IOS_MIN, [
        ('SIMULATOR64', 'x86_64', 'ios-x86_64-simulator'),
        ('SIMULATORARM64', 'arm64', 'ios-arm64-simulator'),
    ]),
    ('watchos-arm64_32_arm64', 'WatchOS', WATCHOS_MIN, [
        ('WATCHOS', 'arm64_32', 'watchos-arm64_32'),
        ('WATCHOS', 'arm64', 'watchos-arm64'),
    ]),
    ('watchos-arm64_x86_64-simulator', 'WatchSimulator', WATCHOS_MIN, [
        ('SIMULATOR_WATCHOS', 'x86_64', 'watchos-x86_64-simulator'),
        ('SIMULATOR_WATCHOS', 'arm64', 'watchos-arm64-simulator'),
    ]),
]

MODULEMAP = '''framework module MarsXlog {
    umbrella header "MarsXlog.h"
    export *
    module * { export * }

    link "z"
    link "c++"
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


def install_dir(platform: str) -> str:
    """ios.toolchain.cmake installs into ${CMAKE_SYSTEM_NAME}.out."""
    return 'watchOS.out' if 'WATCH' in platform.upper() else 'iOS.out'


def build_slice(platform: str, archs: str, deployment_target: str, build_dir: str) -> str:
    """Build one platform with ios.toolchain.cmake, return the merged static lib path."""
    abs_build_dir = os.path.join(SCRIPT_PATH, build_dir)
    clean(os.path.join(SCRIPT_PATH, build_dir))
    os.makedirs(abs_build_dir, exist_ok=True)

    cmd = ('cmake -S "%s" -B "%s" -DCMAKE_BUILD_TYPE=Release '
           '-DCMAKE_TOOLCHAIN_FILE="%s" -DPLATFORM=%s -DARCHS=%s -DDEPLOYMENT_TARGET=%s '
           '-DENABLE_ARC=0 -DENABLE_BITCODE=0 -DENABLE_VISIBILITY=1 '
           '&& make -C "%s" -j%d && make -C "%s" install'
           % (SCRIPT_PATH, abs_build_dir, os.path.join(SCRIPT_PATH, 'ios.toolchain.cmake'),
              platform, archs, deployment_target, abs_build_dir, jobs(), abs_build_dir))
    print('==== building %s (%s) ====\n%s' % (platform, archs, cmd))
    ret = subprocess.call(cmd, shell=True, cwd=abs_build_dir)
    os.chdir(SCRIPT_PATH)
    if ret != 0:
        raise RuntimeError('build %s fail' % platform)

    install_path = os.path.join(abs_build_dir, install_dir(platform))
    src_libs = [os.path.join(install_path, lib) for lib in XLOG_STATIC_LIBS]
    src_libs.append(os.path.join(abs_build_dir, 'zstd', 'libzstd.a'))
    for lib in src_libs:
        if not os.path.isfile(lib):
            raise RuntimeError('missing static library: %s' % lib)

    dst_lib = os.path.join(SCRIPT_PATH, SLICES_PATH, '%s.a' % os.path.basename(build_dir))
    if not libtool_libs(src_libs, dst_lib):  # noqa: F405
        raise RuntimeError('libtool %s fail' % dst_lib)
    return dst_lib


def make_framework(src_lib: str, dst_framework: str, platform: str, min_os: str) -> None:
    """Wrap a static library into a .framework bundle that SwiftPM can import."""
    if os.path.exists(dst_framework):
        shutil.rmtree(dst_framework)
    os.makedirs(os.path.join(dst_framework, 'Headers'))
    os.makedirs(os.path.join(dst_framework, 'Modules'))

    shutil.copy(src_lib, os.path.join(dst_framework, FRAMEWORK_NAME))
    copy_file(os.path.join(SCRIPT_PATH, OBJC_HEADER),  # noqa: F405
              os.path.join(dst_framework, 'Headers', os.path.basename(OBJC_HEADER)))

    with open(os.path.join(dst_framework, 'Modules', 'module.modulemap'), 'w') as f:
        f.write(MODULEMAP)

    with open(os.path.join(dst_framework, 'Info.plist'), 'w') as f:
        f.write(INFO_PLIST.format(name=FRAMEWORK_NAME,
                                  bundle_id=BUNDLE_ID,
                                  platform=platform,
                                  min_os=min_os))


def jobs() -> int:
    return max(1, min(8, os.cpu_count() or 1))


def build_xcframework(tag: str = '') -> str:
    gen_mars_revision_file('comm', tag)  # noqa: F405

    slices_dir = os.path.join(SCRIPT_PATH, SLICES_PATH)
    if os.path.exists(slices_dir):
        shutil.rmtree(slices_dir)
    os.makedirs(slices_dir)

    frameworks = []
    for name, platform, min_os, variants in SLICES:
        libs = [build_slice(slice_platform, archs, min_os,
                            '%s/build/%s' % (BUILD_OUT_PATH, directory))
                for slice_platform, archs, directory in variants]
        lib = libs[0]
        if len(libs) > 1:
            lib = os.path.join(slices_dir, '%s.a' % name)
            if not lipo_libs(libs, lib):  # noqa: F405
                raise RuntimeError('lipo %s fail' % lib)

        framework = os.path.join(slices_dir, 'frameworks', name, '%s.framework' % FRAMEWORK_NAME)
        make_framework(lib, framework, platform, min_os)
        frameworks.append(framework)

    dst = os.path.join(SCRIPT_PATH, DIST_PATH, '%s.xcframework' % FRAMEWORK_NAME)
    remove_if_exist(dst)  # noqa: F405
    cmd = ('xcodebuild -create-xcframework %s -output "%s"'
           % (' '.join('-framework "%s"' % f for f in frameworks), dst))
    print(cmd)
    if subprocess.call(cmd, shell=True) != 0:
        raise RuntimeError('create-xcframework fail')

    print('==================Output========================')
    print(dst)
    return dst


def main() -> int:
    parser = argparse.ArgumentParser(description='Build MarsXlog.xcframework')
    parser.add_argument('tag', nargs='?', default='', help='build tag written into comm/verinfo.h')
    parser.add_argument('--zip',
                        action='store_true',
                        help='zip the xcframework, ready to be attached to a GitHub release')
    args = parser.parse_args()

    xcframework = build_xcframework(args.tag)

    if args.zip:
        zip_path = os.path.join(SCRIPT_PATH, DIST_PATH, '%s.xcframework.zip' % FRAMEWORK_NAME)
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
