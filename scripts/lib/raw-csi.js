'use strict';

const CSI_MAGIC_V1 = 0xC5110001;
const CSI_MAGIC_V2 = 0xC5110006;
const CSI_HEADER_SIZE_V1 = 20;
const CSI_HEADER_SIZE_V2 = 26;
const MAX_ANTENNAS = 4;
const MAX_SUBCARRIERS = 256;

function channelFromFreqMhz(freqMhz) {
  if (freqMhz >= 2412 && freqMhz <= 2472) {
    return Math.round((freqMhz - 2412) / 5) + 1;
  }
  if (freqMhz === 2484) {
    return 14;
  }
  if (freqMhz >= 5005) {
    return Math.round((freqMhz - 5000) / 5);
  }
  return 0;
}

function formatSourceMac(buf, offset) {
  return Array.from(buf.subarray(offset, offset + 6), (byte) =>
    byte.toString(16).padStart(2, '0')).join(':');
}

function getRawCsiLayout(buf) {
  if (!Buffer.isBuffer(buf) || buf.length < CSI_HEADER_SIZE_V1) {
    return null;
  }

  const magic = buf.readUInt32LE(0);
  if (magic === CSI_MAGIC_V1) {
    return { magic, headerSize: CSI_HEADER_SIZE_V1, sourceMac: null };
  }

  if (magic !== CSI_MAGIC_V2 || buf.length < CSI_HEADER_SIZE_V2) {
    return null;
  }

  const nAntennas = buf.readUInt8(5);
  const nSubcarriers = buf.readUInt16LE(6);
  if (
    nAntennas === 0 || nAntennas > MAX_ANTENNAS ||
    nSubcarriers === 0 || nSubcarriers > MAX_SUBCARRIERS
  ) {
    return null;
  }

  const expectedLength = CSI_HEADER_SIZE_V2 + nAntennas * nSubcarriers * 2;
  if (buf.length < expectedLength) {
    return null;
  }

  return {
    magic,
    headerSize: CSI_HEADER_SIZE_V2,
    sourceMac: formatSourceMac(buf, 20),
  };
}

function parseRawCsiFrame(buf, { includePhase = true } = {}) {
  const layout = getRawCsiLayout(buf);
  if (!layout) {
    return null;
  }

  const nodeId = buf.readUInt8(4);
  const nAntennas = buf.readUInt8(5);
  const nSubcarriers = buf.readUInt16LE(6);
  const freqMhz = buf.readUInt32LE(8);
  const seq = buf.readUInt32LE(12);
  const rssiRaw = buf.readInt8(16);
  const noiseFloor = buf.readInt8(17);
  const iqLength = nAntennas * nSubcarriers * 2;
  const expectedLength = layout.headerSize + iqLength;

  if (
    nAntennas === 0 || nAntennas > MAX_ANTENNAS ||
    nSubcarriers === 0 || nSubcarriers > MAX_SUBCARRIERS ||
    buf.length < expectedLength
  ) {
    return null;
  }

  const amplitudes = new Float64Array(nSubcarriers);
  const phases = includePhase ? new Float64Array(nSubcarriers) : null;

  for (let sc = 0; sc < nSubcarriers; sc++) {
    const offset = layout.headerSize + sc * 2;
    const iVal = buf.readInt8(offset);
    const qVal = buf.readInt8(offset + 1);
    amplitudes[sc] = Math.sqrt(iVal * iVal + qVal * qVal);
    if (phases) {
      phases[sc] = Math.atan2(qVal, iVal);
    }
  }

  return {
    magic: layout.magic,
    nodeId,
    nAntennas,
    nSubcarriers,
    freqMhz,
    seq,
    rssi: rssiRaw > 0 ? -rssiRaw : rssiRaw,
    noiseFloor,
    sourceMac: layout.sourceMac,
    headerSize: layout.headerSize,
    channel: channelFromFreqMhz(freqMhz),
    amplitudes,
    phases,
  };
}

module.exports = {
  CSI_MAGIC_V1,
  CSI_MAGIC_V2,
  CSI_HEADER_SIZE_V1,
  CSI_HEADER_SIZE_V2,
  channelFromFreqMhz,
  getRawCsiLayout,
  parseRawCsiFrame,
};
