#!/usr/bin/env python3
import calendar
import datetime as dt
import fcntl
import json
import sys
import time
import backend

MONTHS=['enero','febrero','marzo','abril','mayo','junio','julio','agosto','septiembre','octubre','noviembre','diciembre']
DAYS=['lunes','martes','miércoles','jueves','viernes','sábado','domingo']
FILE=backend.RUN/'calendar.json'

def get():
    today=dt.date.today()
    try:
        state=json.loads(FILE.read_text())
        dt.date(state['year'],state['month'],1)
        dt.date.fromisoformat(state['selected'])
    except (OSError,ValueError,KeyError):
        state=dict(year=today.year,month=today.month,selected=today.isoformat(),today=today.isoformat())
    if state.get('today')!=today.isoformat():
        state=dict(year=today.year,month=today.month,selected=today.isoformat(),today=today.isoformat())
    return state

def save(s):
    tmp=FILE.with_suffix('.tmp')
    tmp.write_text(json.dumps(s)); tmp.replace(FILE)

def status():
    s=get()
    today=dt.date.today()
    selected=dt.date.fromisoformat(s['selected'])
    weeks=[]
    for week in calendar.Calendar(firstweekday=0).monthdatescalendar(s['year'],s['month']):
        weeks.append([dict(day=d.day,id=d.isoformat(),current=d.month==s['month'],today=d==today,selected=d==selected,weekend=d.weekday()>4) for d in week])
    return dict(title=f"{MONTHS[s['month']-1].capitalize()} {s['year']}",weeks=weeks,selected=f'{DAYS[selected.weekday()].capitalize()}, {selected.day} de {MONTHS[selected.month-1]} de {selected.year}')

def action(act,value=''):
    with open(backend.RUN/'calendar.lock','a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        s=get()
        if act=='today':
            FILE.unlink(missing_ok=True); s=get()
        elif act in ('prev','next'):
            index=s['year']*12+s['month']-1+(1 if act=='next' else -1)
            year,month=divmod(index,12)
            if not 1900<=year<=2200: raise ValueError('Mes fuera de rango')
            s['year']=year; s['month']=month+1
        elif act=='select':
            s['selected']=dt.date.fromisoformat(value).isoformat()
        else: raise ValueError('Acción inválida')
        save(s)

if __name__=='__main__':
    if sys.argv[1]=='action':
        action(*sys.argv[2:])
    else:
        last=''
        while True:
            text=json.dumps(status(),ensure_ascii=False)
            if text!=last: print(text,flush=True); last=text
            if sys.argv[1]!='watch': break
            time.sleep(1)
