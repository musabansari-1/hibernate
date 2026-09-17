import { chromium } from 'playwright';
import { resolve } from 'node:path';
import { mkdir } from 'node:fs/promises';
const out=resolve('submission-assets/rendered');
await mkdir(out,{recursive:true});
const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});
const page=await browser.newPage({viewport:{width:1270,height:760},deviceScaleFactor:1});
const file='file:///'+resolve('submission-assets/gallery.html').replaceAll('\\','/');
for(let i=1;i<=4;i++){
  await page.setViewportSize({width:1270,height:760});
  await page.goto(`${file}?slide=${i}`);
  await page.screenshot({path:resolve(out,`contextsleep-gallery-${i}.png`),fullPage:false});
}
await page.setViewportSize({width:240,height:240});
await page.goto(`${file}?slide=thumb`);
await page.screenshot({path:resolve(out,'contextsleep-thumbnail.png'),fullPage:false});
await browser.close();
console.log(out);
