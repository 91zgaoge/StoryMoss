import re
files = [
    'F:/mywork/CINEMA-AI/v2-rust/src-frontend/dist/assets/frontstage-TC5dxCw9.js',
    'F:/mywork/CINEMA-AI/v2-rust/src-frontend/dist/assets/useSyncStore-BlCVHMtl.js'
]
for path in files:
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    matches = re.findall(r'ne\(["\']smart_execute["\'].*?\)', content)
    print(f'=== {path} ===')
    for m in matches[:3]:
        print(m[:200])
    if 'userInput' in content:
        print('WARNING: userInput exists')
    elif 'user_input' in content:
        print('OK: user_input exists')
    else:
        print('Neither userInput nor user_input found')
