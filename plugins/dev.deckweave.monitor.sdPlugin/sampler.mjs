// SPDX-License-Identifier: GPL-3.0-or-later
import os from 'node:os';
import { GpuClient } from './client.mjs';
import { cpuSample, utilization } from './monitor.mjs';

/** One bounded two-second source for every visible key/strip and open metric panel. */
export class SharedSampler {
  constructor(
    publish,
    {
      system = os,
      createGpuClient = () => new GpuClient(),
      every = setInterval,
      cancel = clearInterval,
    } = {},
  ) {
    Object.assign(this, {
      publish,
      system,
      createGpuClient,
      every,
      cancel,
      demand: new Map(),
      interval: null,
      gpuClient: null,
      previous: null,
      sample: {
        cpu: null,
        memory: null,
        usedGiB: 0,
        totalGiB: 0,
        uptime: 0,
        gpu: null,
        gpuError: null,
      },
    });
  }
  setDemand(id, gpu) {
    if (gpu === null) this.demand.delete(id);
    else this.demand.set(id, gpu);
    if (!this.demand.size) {
      if (this.interval !== null) this.cancel(this.interval);
      this.interval = null;
      this.previous = null;
      this.sample.cpu = null;
    }
    const wantsGpu = [...this.demand.values()].some(Boolean);
    if (!wantsGpu && this.gpuClient) {
      this.gpuClient.close();
      this.gpuClient = null;
      this.sample.gpu = null;
      this.sample.gpuError = null;
    }
    if (wantsGpu && !this.gpuClient) {
      this.gpuClient = this.createGpuClient();
      this.collectGpu();
    }
    if (this.demand.size && this.interval === null) {
      this.collect();
      this.interval = this.every(() => this.collect(), 2000);
    }
  }
  collectGpu() {
    const client = this.gpuClient;
    if (!client || this.gpuPending) return;
    this.gpuPending = client;
    void client
      .sample()
      .then(
        (sample) => {
          if (this.gpuClient !== client) return;
          this.sample.gpu = sample;
          this.sample.gpuError = null;
        },
        (error) => {
          if (this.gpuClient !== client) return;
          const reason = String(error.message).slice(0, 300);
          if (reason !== this.sample.gpuError) console.error('[Monitor GPU]', reason);
          this.sample.gpuError = reason;
        },
      )
      .finally(() => {
        if (this.gpuPending === client) this.gpuPending = null;
        if (this.gpuClient === client) this.publish(this.sample);
      });
  }
  collect() {
    if (!this.demand.size) return;
    const next = cpuSample(this.system.cpus()),
      total = this.system.totalmem();
    const used = Math.max(0, total - this.system.freemem());
    Object.assign(this.sample, {
      cpu: this.previous ? utilization(this.previous, next) : null,
      memory: total > 0 ? (used / total) * 100 : null,
      usedGiB: used / 2 ** 30,
      totalGiB: total / 2 ** 30,
      uptime: this.system.uptime(),
    });
    this.previous = next;
    this.publish(this.sample);
    this.collectGpu();
  }
  close() {
    this.demand.clear();
    if (this.interval !== null) this.cancel(this.interval);
    this.interval = null;
    this.gpuClient?.close();
    this.gpuClient = null;
  }
}
