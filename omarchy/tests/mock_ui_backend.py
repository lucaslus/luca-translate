"""Private UI fixture. It never invokes desktop tools, keyrings or remote services."""
import json
import os
from pathlib import Path
import sys
import threading
import time

lock = threading.Lock()
state = {
    'preferences': {'language': 'en'},
    'services': {key: True for key in ['youdao', 'bing', 'google', 'deepl']},
    'ai': {'enabled': False, 'base_url': 'http://localhost:11434/v1', 'model': '', 'has_api_key': False},
    'official': {'enabled': False, 'pro': False, 'has_api_key': False},
    'routing': {'rules': [{'from': 'en', 'to': 'zh-Hans'}, {'from': 'zh-Hans', 'to': 'en'}], 'fallback': 'zh-Hans'},
}
shortcuts = {key: 'Super+Ctrl+Shift+' + letter for key, letter in
             [('input','I'), ('toggle','T'), ('selection','D'), ('screenshot','S'), ('ocr','C'), ('annotate','P')]}
history = [{'id':i, 'text':'record ' + str(i), 'result':'translated record ' + str(i), 'service':'Bing'} for i in range(1, 61)]
favorites = []
counts = {}
last = {}
mode = ''


def emit(value):
    with lock:
        print(json.dumps(value, ensure_ascii=False), flush=True)


def reply(identity, data=None, error=None):
    emit({'id':identity, 'ok':error is None, **({'data':data} if error is None else {'error':error})})


def event(identity, kind, data):
    emit({'event':'translation', 'data':{'request_id':identity, 'kind':kind, 'data':data}})


def translate(identity, params):
    services = [params['only']] if params.get('only') else ['YoudaoDict','Bing']
    event(identity,'start',{'services':services,'from':'en','to':params.get('to','zh-Hans')})
    if params['text'] == 'hang fixture':
        return
    if params['text'] == 'slow fixture':
        time.sleep(0.15)
    for service in services:
        error = params['text'] == 'failure fixture' and service == 'Bing' and not params.get('only')
        data = {'service':service,'text':params['text'],'paragraphs':[] if error else ['translated: ' + params['text']],
                'detected_from':'en','detected_to':'zh-Hans','error':'fixture unavailable' if error else None}
        if service == 'YoudaoDict':
            data['dict'] = {'word':params['text'],'meanings':[['int.','fixture meaning']], 'uk_phonetic':'həˈləʊ','uk_speech':'https://dict.youdao.com/dictvoice?audio=hello&type=1'}
        event(identity,'result',data)
    event(identity,'done',{})
    history.insert(0, {'id':1000+len(history),'text':params['text'],'result':'translated: '+params['text'],'service':'Bing'})


emit({'event':'ready','data':{'protocol':1,'version':'test'}})
for line in sys.stdin:
    message = json.loads(line)
    identity, method, params = message['id'], message['method'], message.get('params') or {}
    counts[method] = counts.get(method,0) + 1
    last[method] = {key:value for key,value in params.items() if key != 'api_key'}
    if method == 'tests.mode':
        mode = params.get('mode',''); reply(identity,{})
    elif method == 'tests.state':
        reply(identity,{'counts':counts,'last':last,'favorites':favorites,'history_count':len(history)})
    elif method == 'tests.crash':
        os._exit(9)
    elif method == 'tests.ignore':
        pass
    elif method == 'settings':
        reply(identity,state)
    elif method in ['preferences.save','routing.save','ai.save','official.save','service.set']:
        if mode == 'reject_save':
            reply(identity,error='fixture rejected save'); continue
        if method == 'preferences.save': state['preferences'] = params
        elif method == 'routing.save': state['routing'] = params
        elif method == 'service.set': state['services'][params['id']] = params['enabled']
        else:
            key = method.split('.')[0]
            state[key].update({name:value for name,value in params.items() if name not in ['api_key','clear_key']})
            if params.get('api_key'): state[key]['has_api_key'] = True
            elif params.get('clear_key'): state[key]['has_api_key'] = False
        reply(identity,state)
    elif method == 'official.test':
        reply(identity,{'connected':True})
    elif method == 'shortcuts.status':
        reply(identity,{'shortcuts':shortcuts,'conflicts':{}})
    elif method == 'shortcuts.save':
        conflicts = {'input':'fixture occupied'} if params['input'] == 'Super+Q' else {}
        shortcuts = {key:('' if key in conflicts else value) for key,value in params.items()}
        reply(identity,{'shortcuts':shortcuts,'conflicts':conflicts})
    elif method == 'translate':
        reply(identity,{'started':True})
        threading.Thread(target=translate,args=(identity,params),daemon=True).start()
    elif method in ['history.list','favorites.list']:
        rows = favorites if method == 'favorites.list' else history
        offset, limit = params.get('offset',0),params.get('limit',50)
        reply(identity,rows[offset:offset+limit])
    elif method == 'history.clear':
        history.clear(); reply(identity,{})
    elif method == 'favorite.add':
        if not any(row['text'] == params['text'] and row['result'] == params['result'] for row in favorites):
            favorites.append({**params,'id':len(favorites)+1})
        reply(identity,{})
    elif method == 'favorite.remove':
        favorites = [row for row in favorites if row['id'] != params['id']]; reply(identity,{})
    elif method == 'capture':
        if mode == 'cancel_capture': reply(identity,{'cancelled':True})
        elif params['action'] == 'ocr': reply(identity,{'copied':True})
        elif params['action'] == 'annotate':
            reply(identity,{'annotation_id':'fixture-image','path':os.environ['NATIVE_TEST_IMAGE'],'output_path':os.environ['NATIVE_TEST_OUTPUT']})
        else: reply(identity,{'text':'captured fixture'})
    elif method == 'diagnostics':
        reply(identity,{'available':True,'write_failed':False,'dropped_events':0})
    else:
        reply(identity,{})
