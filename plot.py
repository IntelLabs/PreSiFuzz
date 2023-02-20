#!/usr/bin/env python
# -*- coding: utf-8 -*-

# import libraries
import time
import matplotlib.pyplot as plt
import pandas as pd
import numpy as np
import re
from datetime import datetime                                                                                                                                                                 

# create an empty dataframe that will store streaming data
df = pd.DataFrame({'run-time': [], 'coverage': [], 'corpus': [], 'objectives': [], 'executions': [], 'throughput': []})

plt.style.use('dark_background')

# create plot
plt.ion() # <-- work in "interactive mode"
plt.show()

fig, ((ax1, ax2, ax3), (ax4, ax5, ax6)) = plt.subplots(2,3)
fig.suptitle('PreSiFuzz')

# act on new data coming from streamer
plot_data = {'run-time': [0], 'coverage': [0], 'corpus': [0], 'objectives': [0], 'executions': [0], 'throughput': [0]}

corpus = 0
objectives = 0
executions = 0
throughput = 0
total_seconds = 0
coverage = 0.0

with open("./fuzzing_from_ram.txt") as f:

    for data in f:

        data = data.rstrip()

        if "(GLOBAL)" in data:
            regex = r"run time: ([\d\d]+h-[\d\d]+m-[\d\d]+s).*corpus: (\d+), objectives: (\d+), executions: (\d+), exec\/sec: ([\d.]+)"

            matches = re.findall(regex, data, re.MULTILINE)
            if len(matches) == 1 and len(matches[0]) == 5:
                matches = matches[0]
                runtime = matches[0]
                corpus = matches[1]
                objectives = matches[2]
                executions = matches[3]
                throughput = matches[4]

                pt = datetime.strptime(runtime,'%Hh-%Mm-%Ss')
                total_seconds = pt.second + pt.minute*60 + pt.hour*3600

        elif "coverage" in data:
            regex = r"VDB: ([\d\w.\/]+)"

            matches = re.findall(regex, data, re.MULTILINE)
            if len(matches) == 1:
                vdb = matches[0]

            regex = r"coverage: ([\d.]+)"

            matches = re.findall(regex, data, re.MULTILINE)
            if len(matches) == 1:
                coverage = matches[0]

        row = {'run-time': [total_seconds], 'coverage': [float(coverage)], 'corpus': [int(corpus)], 'objectives': [int(objectives)], 'executions': [int(executions)], 'throughput': [float(throughput)]}

        plot_data["coverage"].append(max(plot_data["coverage"][-1], row["coverage"][0]))
        plot_data["run-time"]   += row["run-time"]
        plot_data["corpus"]     += row["corpus"]
        plot_data["objectives"] += row["objectives"]
        plot_data["executions"] += row["executions"]
        plot_data["throughput"] += row["throughput"]

# plot all data
ax1.plot(plot_data["run-time"], plot_data["coverage"], color='b')
ax1.set_title('Coverage over time')

ax2.plot(plot_data["run-time"], plot_data["corpus"], color='b')
ax2.set_title('|Corpus| over time')
ax2.set_yscale("log")

ax3.plot(plot_data["run-time"], plot_data["objectives"], color='b')
ax3.set_title('|Objectives| over time')
ax3.set_yscale("log")

ax4.plot(plot_data["run-time"], plot_data["executions"], color='b')
ax4.set_title('Cumulative executions over time')
ax4.set_yscale("log")

ax5.plot(plot_data["run-time"], plot_data["throughput"], color='b')
ax5.set_title('throughput over time')

# show the plot
while 1:
    plt.draw()
    plt.pause(0.0001) # <-- sets the current plot until refreshed
