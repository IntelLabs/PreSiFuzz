#!/usr/bin/env python
# -*- coding: utf-8 -*-

# SPDX-FileCopyrightText: 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

import os
import re
import time
import copy
from datetime import datetime
import matplotlib.pyplot as plt
import numpy as np
from collections import OrderedDict
import pandas
import seaborn as sns
import json
import argparse

print(sns.__version__)


def is_valid_directory(arg):
    if not os.path.isdir(arg):
        raise argparse.ArgumentTypeError(f"'{arg}' is not a valid directory")
    return arg

def parse_args():
    parser = argparse.ArgumentParser(description="Parse command line arguments")
    parser.add_argument("-d", "--directory", type=is_valid_directory, required=True, help="Input directory")
    parser.add_argument("-m", "--metric", choices=["line", "branch", "cond", "tgl", "fsm"], required=True, help="Metric to be provided")
    return parser.parse_args()

args = parse_args()
print("Input directory:", args.directory)
print("Metric:", args.metric)

stats_directories = [args.directory]
metric = args.metric

# first, let's load all the data
data = []
for stats_directory in stats_directories:

    i = 0
    delta = 0
    for stats_filename in os.scandir(stats_directory):

        f = stats_filename
        
        if "stats" not in f.name:
            continue


        print(f"Analyzing file {f.name}: ")

        if os.path.isfile(f):
            f = open(f)
            
            last_runtime = 0
            last_coverage = 0

            lines = f.readlines()
            for l in lines:
   
                l = json.loads(l)

                if  "coverage_verdi_"+metric in l["UpdateUserStats"]["name"]:
                    a = l["UpdateUserStats"]["value"]["value"]["Ratio"][0]
                    b = l["UpdateUserStats"]["value"]["value"]["Ratio"][1]
                    last_coverage = a/b

                if  "time_verdi_"+metric in l["UpdateUserStats"]["name"]:
                    last_runtime = l["UpdateUserStats"]["value"]["value"]["Number"]
                    data += [{"runtime": last_runtime, "score": last_coverage}]
                i += 1

# let's order the timepoint
dataset = []
runtime = []
coverage = []
max_cov = 0.0

time_sorted = sorted(data, key=lambda x: x['runtime'])

delta = time_sorted[0]["runtime"]

for item in time_sorted:
	
    runtime += [item["runtime"]]

    if max_cov < item["score"]:
        max_cov = item["score"]

    coverage += [max_cov]

dataset = {"Execution Time": runtime, "Score": coverage}
print(dataset)
ax = sns.lineplot(x=dataset["Execution Time"], y=dataset["Score"], legend="full")

plt.title(f"{metric} coverage score over time.")
plt.legend(loc='upper center')
plt.savefig(f'{metric}.png')
