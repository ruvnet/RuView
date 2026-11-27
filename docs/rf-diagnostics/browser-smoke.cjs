'use strict';
const assert=require('node:assert/strict'),path=require('node:path');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({headless:true});
 try{
 const page=await browser.newPage({viewport:{width:390,height:844}});
 const errors=[],external=[];
 page.on('pageerror',e=>errors.push(e.message));
 page.on('request',r=>{if(/^https?:/.test(r.url()))external.push(r.url());});
 await page.goto('file://'+path.join(__dirname,'index.html'));
 await page.click('#demo');
 assert.equal(await page.textContent('#mode'),'SYNTHETIC');
 assert.equal(await page.textContent('#count'),'96');
 assert(await page.locator('#chart path').getAttribute('d'));
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 const r=require('./model.js').demo()[0];r.source_kind='live';
 await page.setInputFiles('#file',{name:'live.jsonl',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(r))});
 await page.waitForFunction(()=>document.querySelector('#mode').textContent==='LIVE UNVERIFIED');
 assert.match(await page.textContent('#status'),/file replay/);
 await page.setInputFiles('#file',{name:'bad.jsonl',mimeType:'application/json',buffer:Buffer.from('{"iq":[1,2]}')});
 await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('Import rejected'));
 assert.equal(await page.textContent('#count'),'0');
 await page.click('#demo');await page.click('#clear');
 assert.equal(await page.textContent('#count'),'0');
 assert.deepEqual(errors,[]);assert.deepEqual(external,[]);
 console.log('PASS mobile render, demo, JSONL import, unverified live label, malformed rejection, clear, zero external requests');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
