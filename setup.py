#!/usr/bin/env python3
# -*- coding: utf-8 -*-

from setuptools import find_packages, setup

with open('README.md', 'rb') as f:
    readme = f.read().decode('utf-8')

setup(
    name='bigtube',
    scripts=['bigtube'],
    version='1.0.0',
    description='BigTube Application',
    long_description=readme,
    long_description_content_type="text/markdown",
    author='eltonff',
    author_email='eltonfabricio10@gmail.com',
    url='https://github.com/eltonfabricio10/bigtube',
    install_requires=['yt-dlp', 'PyGObject'],
    license='MIT',
    keywords=['big', 'tube', 'biglinux', 'video', 'audio', 'download'],
    packages=find_packages(),
    include_package_data=True,
    classifiers=[
        'Intended Audience :: Developers',
        'Natural Language :: Portuguese (Brazilian)',
        'Programming Language :: Python :: 3',
        'Operating System :: POSIX :: Linux',
    ],
)
