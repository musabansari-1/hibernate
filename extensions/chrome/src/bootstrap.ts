async function run(){
 try{await chrome.runtime.sendMessage({type:'connect'});}catch{}
 const tab=await chrome.tabs.getCurrent();
 if(tab?.id!==undefined)setTimeout(()=>void chrome.tabs.remove(tab.id!).catch(()=>{}),750);
}
void run();
