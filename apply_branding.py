# -*- coding: utf-8 -*-
import os
import re

# Header Templates
TEMPLATES = {
    'js': u"""/**
 * HUE ENGINE ⬢
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */
""",
    'rs': u"""/*
 * HUE ENGINE ⬢
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */
""",
    'css': u"""/*
 * HUE ENGINE ⬢
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */
""",
    'html': u"""<!--
  HUE ENGINE ⬢
  System Designation: The Architecture of Aggregation

  @author roywalk3r <roywalker440@gmail.com>
  @repository https://github.com/roywalk3r/hue.git
  @license MIT
-->
""",
    'sh': u"""# HUE ENGINE ⬢
# System Designation: The Architecture of Aggregation
#
# @author roywalk3r <roywalker440@gmail.com>
# @repository https://github.com/roywalk3r/hue.git
# @license MIT
""",
    'toml': u"""# HUE ENGINE ⬢
# System Designation: The Architecture of Aggregation
#
# @author roywalk3r <roywalker440@gmail.com>
# @repository https://github.com/roywalk3r/hue.git
# @license MIT
""",
    'py': u"""# HUE ENGINE ⬢
# System Designation: The Architecture of Aggregation
#
# @author roywalk3r <roywalker440@gmail.com>
# @repository https://github.com/roywalk3r/hue.git
# @license MIT
""",
}

# Markdown special handling
MD_SPEC = u"""# HUE ENGINE ⬢
### System Designation: The Architecture of Aggregation
"""

EXCLUDE_DIRS = {
    'node_modules', 'target', '.git', '.claude', '.omc', '.omx', 
    '.superpowers', '.worktrees', 'vendor', '.playwright', 
    '.playwright-cli', '.pnpm-store'
}

EXTENSIONS = {'js', 'rs', 'css', 'html', 'sh', 'toml', 'py', 'md'}

def apply_branding(file_path):
    ext = file_path.split('.')[-1]
    if ext not in EXTENSIONS:
        return

    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            content = f.read()
    except Exception:
        return

    # Skip files that already have the correct new branding
    if u"HUE ENGINE ⬢" in content and u"Architecture of Aggregation" in content:
        return

    new_content = content
    header = TEMPLATES.get(ext)

    if ext == 'md':
        # Special case for MD
        if content.startswith('# HUE STREAM API') or content.startswith('# HUE ENGINE'):
            # Replace first two lines or so
            lines = content.split('\n')
            if len(lines) > 0 and (lines[0].startswith('# HUE')):
                lines[0] = u"# HUE ENGINE ⬢"
                if len(lines) > 1 and ("System Designation" in lines[1] or "###" in lines[1]):
                    lines[1] = u"### System Designation: The Architecture of Aggregation"
                new_content = '\n'.join(lines)
        else:
            new_content = MD_SPEC + "\n" + content
    else:
        # Check for existing header blocks
        patterns = [
            r'/\*\*.*?\*/',        # JS Doc style
            r'/
*.*?
*/',          # C style
            r'(^//[!/].*?\n)+',    # Rust doc or line comments
            r'(^#.*?\n)+',         # Shell/Toml comments
            r'<!--.*?-->'
        ] # HTML comments
        
        found_old = False
        for pattern in patterns:
            match = re.search(pattern, content, re.DOTALL | re.MULTILINE)
            if match and match.start() == 0:
                # Check if it looks like our old header
                block = match.group(0)
                if 'roywalk3r' in block or 'Hue Stream' in block or 'HUE STREAM' in block or 'HUE ENGINE' in block:
                    new_content = header + "\n" + content[match.end():].lstrip()
                    found_old = True
                    break
        
        if not found_old:
            # Handle shebang
            if content.startswith('#!'):
                lines = content.split('\n', 1)
                if len(lines) > 1:
                    new_content = lines[0] + "\n\n" + header + "\n" + lines[1]
                else:
                    new_content = lines[0] + "\n\n" + header
            else:
                new_content = header + "\n" + content

    if new_content != content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(new_content)
        print(f"Updated: {file_path}")

def main():
    root_dir = "/mnt/devdrive/projects/Misc/hue"
    for root, dirs, files in os.walk(root_dir):
        # Filter excluded directories
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        
        for file in files:
            file_path = os.path.join(root, file)
            try:
                apply_branding(file_path)
            except Exception as e:
                print(f"Error processing {file_path}: {e}")

if __name__ == "__main__":
    main()
