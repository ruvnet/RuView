'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),M=require('./model.js');
const good=()=>M.demo()[0];
test('synthetic fixture and JSONL roundtrip',()=>assert.equal(M.parse(M.demo().map(JSON.stringify).join('\n')).length,96));
test('captured and live labels remain unverified',()=>{for(const source_kind of ['live','captured_replay'])assert.equal(M.validate({...good(),source_kind}).valid,false);});
test('reject malformed, empty, oversized and too many records',()=>{
 for(const s of ['', '[]','{',' '.repeat(M.MAX_BYTES+1),Array(10002).fill('{}').join('\n')])assert.throws(()=>M.parse(s));
});
test('reject inference promotion, raw IQ, nonfinite metrics and unsafe indices',()=>{
 for(const patch of [{valid:true},{config_verified:true},{iq:[1,2]},{rms_dbfs:Infinity},{rail_fraction:2},{rail_fraction:NaN},{observed_monotonic_ns:2**53},{source_kind:'__proto__'},{ttl_ms:0},{invalid_reasons:['<script>']},{invalid_reasons:[]}])assert.throws(()=>M.validate({...good(),...patch}));
});
test('reject missing fields with record index',()=>{
 const r=good();delete r.sample_start;assert.throws(()=>M.parse(JSON.stringify(r)),/Record 1: Missing field/);
});
test('allow unavailable measurements but retain invalid evidence',()=>{
 assert.equal(M.validate({...good(),rms_dbfs:null,rail_fraction:null,center_frequency_hz:null,sample_rate_hz:0}).valid,false);
});

test('match producer and RuField numeric contract',()=>{
 for(const patch of [{sample_start:0},{sample_count:4096},{missing_samples:128},{sample_rate_hz:123},{center_frequency_hz:1999999999},{center_frequency_hz:8000000001},{rms_dbfs:17},{rms_dbfs:-201},{rms_dbfs:-6,rail_fraction:null},{invalid_reasons:['configuration_unverified','configuration_unverified']},{invalid_reasons:['configuration_unverified','other']}])
 assert.throws(()=>M.validate({...good(),...patch}));
 for(const patch of [{center_frequency_hz:7576000000},{rms_dbfs:15.05},{sample_count:0,rms_dbfs:null,rail_fraction:null},{sample_rate_hz:0},{missing_samples:2048}])M.validate({...good(),...patch});
});
