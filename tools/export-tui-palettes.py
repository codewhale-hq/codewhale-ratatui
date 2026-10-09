#!/usr/bin/env python3
"""Export the current Codewhale palette constants without executing source.

Usage: export-tui-palettes.py CODEWHALE_CHECKOUT [--check]
"""
import argparse
import json
import re
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
source = args.source / 'crates/palette/src'
texts = {name: re.sub(r'//[^\n]*', '', (source / name).read_text())
         for name in ('rgb.rs', 'tokens.rs', 'themes.rs')}
constants = {}
for name in ('rgb.rs', 'tokens.rs'):
    constants.update(re.findall(r'pub const (\w+):[^=]+?=\s*(.*?);', texts[name], re.S))

def value(expression):
    expression = expression.strip().rstrip(',').strip()
    if re.fullmatch(r'(0x[\da-fA-F_]+|[\d_]+)', expression):
        return int(expression.replace('_', ''), 16 if expression.startswith('0x') else 10)
    if expression.startswith('Color::Rgb(') and expression.endswith(')'):
        return list(value(part) for part in expression[11:-1].split(',') if part.strip())
    if expression.startswith('(') and expression.endswith(')'):
        return tuple(value(part) for part in expression[1:-1].split(',') if part.strip())
    component = re.fullmatch(r'(\w+)\.([012])', expression)
    if component:
        return value(constants[component[1]])[int(component[2])]
    if expression.startswith('Color::'):
        return expression
    if expression in constants:
        return value(constants[expression])
    raise ValueError(f'unsupported palette constant: {expression}')

variants = ['Underwater', 'UnderwaterRetro', 'Shoreline', 'ShorelineLight',
            'Whale', 'WhaleLight', 'Terminal', 'Grayscale', 'CatppuccinMocha',
            'TokyoNight', 'Dracula', 'GruvboxDark', 'Claude', 'Matrix', 'SolarizedLight', 'Uwu']
blocks = {}
for match in re.finditer(r'pub const (\w+): UiTheme = UiTheme \{(.*?)\}\s*(\.with_terminal_native_shell\(\))?;', texts['themes.rs'], re.S):
    fields = {}
    for field in re.finditer(r'(\w+):\s*("[^"]*"|PaletteMode::\w+|Color::Rgb\([^)]*\)|\w+(?:::\w+)?),', match[2], re.S):
        name, expression = field.groups()
        fields[name] = expression.strip('"') if name == 'name' else expression if name == 'mode' else value(expression)
    if match[3]:
        for key in ('surface_bg', 'panel_bg', 'composer_bg', 'header_bg', 'footer_bg'):
            fields[key] = 'Color::Reset'
    blocks[match[1]] = fields
names = dict(re.findall(r'Self::(\w+) => (\w+),', texts['themes.rs']))
mapping = ['header_bg', 'surface_bg', 'panel_bg', 'elevated_bg', 'selection_bg',
           'text_body', 'text_muted', 'border', 'text_soft', 'accent_primary',
           'surface_bg', 'accent_secondary', 'accent_action', 'error_fg',
           'text_hint', 'text_dim', 'diff_added_bg', 'diff_deleted_bg']
records = []
for variant in variants:
    fields = blocks[names[variant]]
    records.append({'variant': variant, 'name': fields['name'], 'light': fields['mode'] in ('PaletteMode::Light', 'PaletteMode::SolarizedLight'),
                    'roles': [fields[field] for field in mapping], 'slots': fields})

def color(c):
    return f'Color::Rgb({c[0]}, {c[1]}, {c[2]})' if isinstance(c, list) else c

inks = {'Working':'status_working','Success':'success','Warning':'warning','PermissionAsk':'permission_ask','PermissionAutoReview':'permission_auto_review','PermissionFullAccess':'permission_full_access','ModeWork':'mode_agent','ModePlan':'mode_plan','ModeOperate':'mode_operate','Soft':'text_soft','Info':'info'}
rust = ['// Generated from Codewhale crates/palette/src. Use tools/export-tui-palettes.py.',
        'use ratatui::style::Color;', 'use crate::Role;',
        '/// Named palettes already shipped by the Codewhale TUI.',
        '#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]', 'pub enum TuiPalette {']
for index, record in enumerate(records):
    if index == 0: rust.append('    #[default]')
    rust.append(f'    {record["variant"]},')
rust.extend(['}', 'impl TuiPalette {', '    pub const ALL: [Self; 16] = ['])
rust.extend(f'        Self::{record["variant"]},' for record in records)
rust.extend(['    ];', '    pub const fn name(self) -> &\'static str {', '        match self {'])
rust.extend(f'            Self::{r["variant"]} => "{r["name"]}",' for r in records)
rust.extend(['        }', '    }', '    pub const fn light(self) -> bool {', '        matches!(self, Self::ShorelineLight | Self::WhaleLight | Self::SolarizedLight)', '    }', '    pub const fn color(self, role: Role) -> Color {', '        let colors = match self {'])
rust.extend(f'            Self::{r["variant"]} => [{", ".join(color(c) for c in r["roles"])}],' for r in records)
rust.extend(['        };', '        colors[role.index()]', '    }', '    pub const fn ink(self, ink: TuiInk) -> Color {', '        let colors = match self {'])
rust.extend(f'            Self::{r["variant"]} => [{", ".join(color(r["slots"][field]) for field in inks.values())}],' for r in records)
rust.extend(['        };', '        colors[ink as usize]', '    }', '}', '/// Distinct ink slots already used by Codewhale\'s TUI.', '#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]', 'pub enum TuiInk {'])
rust.extend(f'    {ink},' for ink in inks)
rust.extend(['}', 'impl TuiInk {', '    pub const fn fallback_role(self) -> Role {', '        match self {', '            Self::Working | Self::Success => Role::Live,', '            Self::Warning | Self::PermissionAsk | Self::PermissionAutoReview | Self::PermissionFullAccess => Role::Attention,', '            Self::ModeWork | Self::ModePlan | Self::ModeOperate => Role::Primary,', '            Self::Soft => Role::Muted,', '            Self::Info => Role::Primary,', '        }', '    }', '}'])
grounds={'Surface':'surface_bg','Panel':'panel_bg','Elevated':'elevated_bg','Composer':'composer_bg','Selection':'selection_bg','Header':'header_bg','Footer':'footer_bg','DiffAdded':'diff_added_bg','DiffRemoved':'diff_deleted_bg'}
rust.extend(['/// Native background slots, retained independently of the shared role theme.','#'+'[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]','pub enum TuiGround {'])
rust.extend(f'    {ground},' for ground in grounds)
rust.extend(['}', 'impl TuiGround {', '    pub const fn fallback_role(self)->Role {', '        match self {','            Self::Surface | Self::Footer => Role::Background,','            Self::Panel | Self::Composer => Role::Surface,','            Self::Elevated => Role::Hover,','            Self::Selection => Role::Selected,','            Self::Header => Role::Sidebar,','            Self::DiffAdded => Role::DiffAddedTint,','            Self::DiffRemoved => Role::DiffRemovedTint,','        }','    }','}', 'impl TuiPalette {','    pub const fn ground(self,ground:TuiGround)->Color {','        let colors=match self {'])
rust.extend(f'            Self::{r["variant"]} => [{", ".join(color(r["slots"][field]) for field in grounds.values())}],' for r in records)
rust.extend(['        };','        colors[ground as usize]','    }','}'])
formatted = subprocess.run(['rustfmt', '--edition', '2024', '--emit', 'stdout'], input='\n'.join(rust)+'\n', text=True, capture_output=True, check=True).stdout
outputs = {root/'src/tui_palettes.rs': formatted, root/'assets/tui-palettes.json': json.dumps(records, indent=2)+'\n'}
for path, data in outputs.items():
    if args.check:
        if path.read_text() != data: raise SystemExit(f'stale palette export: {path.name}')
    else: path.write_text(data)
print('Exported all 16 fixed Codewhale TUI palettes' if not args.check else 'Codewhale palette export matches source')
