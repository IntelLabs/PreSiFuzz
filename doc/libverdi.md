<!--
SPDX-FileCopyrightText: 2022 Intel Corporation

SPDX-License-Identifier: Apache-2.0
-->

# LIBVERDI

VCS is a commercial simulator modeling RTL code into an executable event-based
model.  This model can produce different coverage information (e.g., tgl, line,
branch, fsm) usefull for Feedback Guided Fuzzing. The purpose of this
library is to provide the required logic to extract such coverage information.
It requires VERDI to be installed, including libNPI.so. 
