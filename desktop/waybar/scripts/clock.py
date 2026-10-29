#!/usr/bin/env python3
"""Spanish clock independent of generated system locales."""
from datetime import datetime

now = datetime.now()
days = ('lun', 'mar', 'mié', 'jue', 'vie', 'sáb', 'dom')
months = ('ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sep', 'oct', 'nov', 'dic')
print(f'{days[now.weekday()]} {now.day:02} {months[now.month - 1]}  {now:%H:%M}')
