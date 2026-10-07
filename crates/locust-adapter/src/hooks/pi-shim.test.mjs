import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, chmodSync, existsSync, appendFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import { setTimeout as sleep } from 'node:timers/promises';

const directory = dirname(fileURLToPath(import.meta.url));
const template = readFileSync(join(directory, 'pi-shim.ts'), 'utf8');
const failureLine = 'Locust context was NOT injected';
const line = 'Locust: goal 123, attempt 456, generation 1: claim lost. Use locust_pending and locust_context_read.';

async function fixture({ body = {line, keep_going:false}, timeout = 5000, hang = false, launcherMissing = false } = {}) {
    const root = mkdtempSync('/tmp/lh.');
    const launcher = join(root, "launcher ' $(touch injected) `touch injected` ${HOME} $& __LOCUST_HOOK_TIMEOUT_MS__ __LOCUST_FAILURE_LINE_JSON__.mjs");
    const record = join(root, 'calls.jsonl');
    const config = join(root, 'reply.json');
    writeFileSync(config, JSON.stringify({body,hang}));
    if (!launcherMissing) {
        writeFileSync(launcher, `#!/opt/homebrew/bin/node\nimport {readFileSync,appendFileSync,watch} from 'node:fs';
const record=${JSON.stringify(record)}, config=${JSON.stringify(config)};
let input='';process.stdin.setEncoding('utf8');process.stdin.on('data',chunk=>input+=chunk);
process.stdin.on('end',()=>{
 const request=JSON.parse(input);
 appendFileSync(record,JSON.stringify({pid:process.pid,home:process.env.HOME,args:process.argv.slice(2),input:request})+'\\n');
 const reply=JSON.parse(readFileSync(config,'utf8'));
 if(reply.hang){const watcher=watch(config,()=>{const next=JSON.parse(readFileSync(config,'utf8'));if(next.hang)return;watcher.close();if(next.body!==null)process.stdout.write(typeof next.body==='string'?next.body:JSON.stringify(next.body));});return;}
 if(reply.body!==null)process.stdout.write(typeof reply.body==='string'?reply.body:JSON.stringify(reply.body));
});\n`);
        chmodSync(launcher, 0o700);
    }
    const code = template.replaceAll('__LOCUST_HOOK_TIMEOUT_MS__', () => String(timeout))
        .replaceAll('__LOCUST_FAILURE_LINE_JSON__', () => JSON.stringify(failureLine))
        .replaceAll('__LOCUST_LAUNCHER_JSON__', () => JSON.stringify(launcher));
    const js = stripTypeScriptTypes(code, {mode:'strip'});
    const factory = (await import('data:text/javascript,' + encodeURIComponent(js) + '#' + root)).default;
    const handlers = new Map(); const messages=[];
    factory({on:(name,handler)=>{assert(!handlers.has(name));handlers.set(name,handler);return()=>handlers.delete(name);},
        sendMessage:(message,options)=>messages.push({message,options})});
    let session='native-chat'; const abort=new AbortController();
    const ctx={sessionManager:{getSessionId:()=>session},signal:abort.signal};
    const invoke=(name,event={})=>handlers.get(name)({type:name,...event},ctx);
    const calls=()=>existsSync(record)?readFileSync(record,'utf8').trim().split('\n').filter(Boolean).map(JSON.parse):[];
    const change=(value)=>writeFileSync(config,JSON.stringify({body:value,hang:false}));
    async function waitForCall() {
        const deadline=Date.now()+5000;
        while(calls().length===0&&Date.now()<deadline)await sleep(5);
        assert(calls().length>0,'fake launcher did not receive stdin');
        return calls().at(-1);
    }
    const alive=(pid)=>{try{process.kill(pid,0);return true;}catch(error){if(error.code==='ESRCH')return false;throw error;}};
    return {root,launcher,record,config,handlers,messages,ctx,abort,invoke,calls,change,waitForCall,alive,setSession:(value)=>session=value};
}

function result(overrides={}) {
    return {toolName:'mcp__locust__locust_wait',toolCallId:'call-1',input:{goal:'11'.repeat(32),seen:0,timeout_ms:0},
        content:[{type:'text',text:'original'},{type:'image',data:'original-image',mimeType:'image/png'}],
        details:{server:'locust',tool:'locust_wait',extra:{preserve:true}},
        structuredContent:{isError:false,content:[{type:'text',text:'{"ok":true,"result":{"waited":"no_event"}}'}],
            structuredContent:{ok:true,result:{waited:'no_event'}}},isError:false,usage:{input:2,output:3},...overrides};
}

function settle(overrides={}) {
    return {entries:[{type:'custom',customType:'other',data:{preserve:true}}],continue:false,
        context:{canContinue:false,preserve:true},outcome:'completed',...overrides};
}

test('start and compaction use native identity without triggering a provider turn',async()=>{
    const f=await fixture();
    assert.deepEqual([...f.handlers.keys()],['session_start','session_compact','session_shutdown','tool_result','agent_before_settle']);
    await f.invoke('session_start',{reason:'resume',previousSessionFile:'/should/not/read'});
    await f.invoke('session_compact',{reason:'threshold',compactionEntry:{summary:'never pass'},fromExtension:false,willRetry:true});
    assert.equal(f.messages.length,2);
    for(const message of f.messages){assert.equal(message.message.content,line);assert.deepEqual(message.options,{triggerTurn:false});}
    const calls=f.calls();assert.equal(calls.length,2);
    assert.deepEqual(calls.map(c=>c.args),[['hook','start','--harness','pi'],['hook','start','--harness','pi']]);
    assert.equal(calls[0].input.session_id,'native-chat');assert.equal(calls[0].input.source,'resume');assert.equal(calls[1].input.source,'compact');
    assert(!JSON.stringify(calls).includes('never pass'));assert(!JSON.stringify(calls).includes('/should/not/read'));
    assert(calls.every(call=>call.home.startsWith('/tmp/lh.')));
});

test('tool result appends one line while preserving all original payload fields',async()=>{
    const f=await fixture();const event=result({parentToolCallId:'script-call',toolCallId:'script-call/1'});
    const before=structuredClone(event);const value=await f.invoke('tool_result',event);
    assert.deepEqual(event,before);
    assert.deepEqual(value.content,[...event.content,{type:'text',text:line}]);
    for(const key of ['details','structuredContent','isError','usage'])assert.deepEqual(value[key],event[key]);
    const call=f.calls()[0];assert.deepEqual(call.args,['hook','tool','--harness','pi']);
    assert.equal(call.input.tool_use_id,'script-call/1');assert.equal(call.input.parent_tool_call_id,'script-call');
    assert.deepEqual(call.input.tool_response.structuredContent,event.structuredContent);
    assert.deepEqual(call.input.tool_response.details,{server:'locust',tool:'locust_wait'});
    assert.equal(call.input.tool_response.isError,false);assert(!('content' in call.input.tool_response));
    assert(!existsSync(join(f.root,'injected')));assert(!existsSync(join(directory,'injected')));
});

test('unrelated and failed tools still poll without changing error or machine result',async()=>{
    const f=await fixture();const event=result({toolName:'bash',isError:true,details:{preserve:true},structuredContent:{error:{code:9}}});
    const value=await f.invoke('tool_result',event);
    assert.equal(value.isError,true);assert.deepEqual(value.structuredContent,event.structuredContent);assert.deepEqual(value.details,event.details);
    assert.equal(f.calls()[0].input.tool_response.isError,true);assert.equal(f.calls()[0].input.tool_name,'bash');
});

test('normal settlement can continue after context was initially not runnable',async()=>{
    const f=await fixture({body:{line,keep_going:true}});const event=settle();const before=structuredClone(event);
    const value=await f.invoke('agent_before_settle',event);
    assert.deepEqual(event,before);assert.equal(value.continue,true);
    assert.deepEqual(value.entries,[...event.entries,{type:'custom_message',customType:'locust',content:line,display:true}]);
    assert.deepEqual(f.calls()[0].args,['hook','stop','--harness','pi']);
    f.change({line,keep_going:false});const other=settle({continue:true});
    assert.equal((await f.invoke('agent_before_settle',other)).continue,true);
});

test('informational settlement preserves entries without creating continuation',async()=>{
    const f=await fixture();const value=await f.invoke('agent_before_settle',settle());
    assert.equal(value.continue,false);assert.equal(value.entries.length,2);
    assert.equal(value.entries[0].customType,'other');
});

test('abort and error settlement never launch or resurrect a turn',async()=>{
    const f=await fixture();
    for(const outcome of ['aborted','error'])assert.equal(await f.invoke('agent_before_settle',settle({outcome})),undefined);
    f.abort.abort();assert.equal(await f.invoke('agent_before_settle',settle()),undefined);
    assert.equal(f.calls().length,0);assert.equal(f.messages.length,0);
});

test('no output and LOCUST_HOOKS off leave native events unchanged',async()=>{
    const f=await fixture({body:null});assert.equal(await f.invoke('tool_result',result()),undefined);
    assert.equal(await f.invoke('agent_before_settle',settle()),undefined);
    const count=f.calls().length;process.env.LOCUST_HOOKS='off';
    try{assert.equal(await f.invoke('tool_result',result()),undefined);assert.equal(await f.invoke('agent_before_settle',settle()),undefined);}
    finally{delete process.env.LOCUST_HOOKS;}
    assert.equal(f.calls().length,count);
});

test('abort during hook wait reaps owned child and injects nothing',async()=>{
    const f=await fixture({hang:true});const pending=f.invoke('agent_before_settle',settle());
    const call=await f.waitForCall();assert(f.alive(call.pid));f.abort.abort();
    assert.equal(await pending,undefined);assert(!f.alive(call.pid));assert.equal(f.messages.length,0);
});

test('shutdown during wait reaps owned child before shutdown resolves',async()=>{
    const f=await fixture({hang:true});const pending=f.invoke('tool_result',result());const call=await f.waitForCall();
    await f.invoke('session_shutdown',{reason:'reload'});assert(!f.alive(call.pid));assert.equal(await pending,undefined);
    assert.equal(await f.invoke('tool_result',result()),undefined);assert.equal(f.calls().length,1);
});

test('adapter timeout reaps child and emits generated fixed failure line',async()=>{
    const f=await fixture({hang:true,timeout:1000});const pending=f.invoke('tool_result',result());const call=await f.waitForCall();
    const value=await pending;assert(!f.alive(call.pid));assert.equal(value.content.at(-1).text,failureLine);assert.equal(value.isError,false);
});

test('late output cannot be injected into a changed session identity',async()=>{
    const f=await fixture({hang:true});const pending=f.invoke('tool_result',result());await f.waitForCall();f.setSession('different-chat');
    f.change({line,keep_going:false});assert.equal(await pending,undefined);
});

test('invalid core output and spawn failure preserve payload and report only generated failure line',async()=>{
    const invalid=['not-json',{line:'contains\nnewline',keep_going:false},{line:'unsafe',keep_going:true},
        {line:null,keep_going:true},{line:'unicode-é',keep_going:false},{line:'x'.repeat(512),keep_going:false},
        {line:'okay',keep_going:'false'},{line:'okay',keep_going:false,unexpected:true}];
    const f=await fixture();
    for(const body of invalid){f.change(body);const event=result();const value=await f.invoke('tool_result',event);
        assert.equal(value.content.at(-1).text,failureLine);assert.equal(value.isError,event.isError);assert.deepEqual(value.structuredContent,event.structuredContent);}
    const missing=await fixture({launcherMissing:true});const value=await missing.invoke('tool_result',result());assert.equal(value.content.at(-1).text,failureLine);
});
