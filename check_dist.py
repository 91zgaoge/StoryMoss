import re

with open('F:/mywork/CINEMA-AI/v2-rust/src-frontend/dist/assets/main-B1MK1mFu.js', 'r', encoding='utf-8') as f:
    content = f.read()

matches = re.findall(r'ne\(["\']smart_execute["\'].*?\)', content)
for m in matches[:5]:
    print(m[:200])
print('---')
if 'userInput' in content:
    print('WARNING: userInput still exists in dist')
else:
    print('OK: userInput not found in dist')
if 'user_input' in content:
    print('OK: user_input found in dist')
else:
    print('WARNING: user_input not found in dist')
