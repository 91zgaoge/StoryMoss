import re

with open('F:/mywork/CINEMA-AI/v2-rust/src-tauri/src/lib.rs', 'r', encoding='utf-8') as f:
    content = f.read()

content = re.sub(r'#\[tauri::command\]\n', '#[tauri::command(rename_all = "snake_case")]\n', content)

with open('F:/mywork/CINEMA-AI/v2-rust/src-tauri/src/lib.rs', 'w', encoding='utf-8') as f:
    f.write(content)

count = len(re.findall(r'#\[tauri::command\(rename_all = "snake_case"\)\]', content))
print(f'Replaced {count} commands')
