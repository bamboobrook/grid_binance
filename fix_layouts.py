import os
import re

base_dir = 'apps/web/app'

def fix_layouts():
    for root, dirs, files in os.walk(base_dir):
        for file in files:
            if not file.endswith('.tsx'):
                continue
            path = os.path.join(root, file)
            with open(path, 'r', encoding='utf-8') as f:
                content = f.read()

            new_content = content
            
            # Layout fixes for Cards
            new_content = new_content.replace('className="content-grid content-grid--metrics"', 'className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 mb-6"')
            new_content = new_content.replace('className="content-grid content-grid--split"', 'className="grid grid-cols-1 xl:grid-cols-2 gap-6 mb-6"')
            new_content = new_content.replace('className="content-grid"', 'className="grid grid-cols-1 xl:grid-cols-2 gap-6 mb-6"')
            
            # DataTable fixes
            if '<DataTable' in new_content and 'overflow-x-auto' not in new_content:
                def replace_datatable(m):
                    return f'<div className="overflow-x-auto whitespace-nowrap min-w-full pb-4 rounded-lg">\n                {m.group(0)}\n              </div>'
                new_content = re.sub(r'<DataTable[\s\S]*?/>', replace_datatable, new_content)

            # FormStack fixes for wide forms
            if '<FormStack' in new_content and 'overflow-x-auto' not in new_content and 'AdminSystemPage' in content:
                new_content = new_content.replace('<FormStack', '<div className="overflow-x-auto whitespace-nowrap min-w-full pb-4">\n<FormStack')
                new_content = new_content.replace('</FormStack>', '</FormStack>\n</div>')

            if new_content != content:
                with open(path, 'w', encoding='utf-8') as f:
                    f.write(new_content)
                print(f"Fixed layout in {path}")

fix_layouts()
