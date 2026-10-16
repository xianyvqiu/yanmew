"""从唯一结构来源生成离线数据库类图；仅使用 Python 标准库，不读取用户数据库。"""
from pathlib import Path
import hashlib
import html
import json
import re
import sqlite3

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / 'db/schema.sql'
OUT = ROOT / 'docs'
GROUPS = [
    ('人设与全局配置', '#6366f1', ['personas', 'pet_artworks', 'sessions', 'session_pet_states', 'user_profiles', 'providers', 'provider_models', 'provider_roles', 'embedding_providers', 'settings']),
    ('聊天、记忆与检索', '#0284c7', ['chat_rounds', 'messages', 'message_images', 'short_memories', 'long_memories', 'long_memory_sources', 'long_memory_embeddings', 'long_memories_fts', 'notebooks']),
    ('出行与旅行内容', '#0d9488', ['travels', 'travel_journals', 'letters', 'accessories', 'souvenirs']),
    ('小游戏插件与钱包', '#d97706', ['game_plugins', 'game_permissions', 'game_plugin_settings', 'game_plugin_data', 'game_runs', 'currency_ledger', 'user_state']),
    ('提醒与节日', '#db2777', ['reminders', 'reminder_occurrences', 'reminder_email_deliveries', 'reminder_email_items', 'festival_catalog', 'custom_festivals']),
]
NAMES = dict(zip(
    'personas sessions user_profiles providers embedding_providers settings chat_rounds messages short_memories long_memories long_memory_sources long_memory_embeddings long_memories_fts notebooks travels travel_journals letters accessories souvenirs game_plugins game_permissions game_plugin_settings game_plugin_data game_runs currency_ledger user_state reminders reminder_occurrences festival_catalog custom_festivals'.split(),
    '人设 会话 用户设定 聊天及记忆提供商 向量模型提供商 全局设置 聊天回合 消息原文 短期记忆 长期记忆 长期记忆来源 向量缓存 全文检索索引 会话记事本 出行记录 旅行游记 信件 饰品 文字伴手礼 插件登记 插件授权 插件设置 插件存档 历史游戏局 喵币账单 全局钱包 提醒规则 提醒触发记录 内置节日 全局自定义日期'.split()))
NAMES['pet_artworks'] = '桌宠创作条目'
NAMES['session_pet_states'] = '会话桌宠状态'
NAMES['providers'] = '通用提供商连接'
NAMES['provider_roles'] = '模型用途绑定'
NAMES['provider_models'] = '连接的各用途模型配置'
NAMES['message_images'] = '聊天图片附件'
NAMES['reminder_email_deliveries'] = '邮件发送任务'
NAMES['reminder_email_items'] = '邮件提醒关联与快照'


def extract():
    source = SCHEMA.read_text(encoding='utf-8-sig')
    con = sqlite3.connect(':memory:')
    con.execute('PRAGMA foreign_keys=ON')
    con.executescript(source)
    names = re.findall(r'CREATE (?:VIRTUAL )?TABLE IF NOT EXISTS (\w+)', source)
    assert set(names) == {t for _, _, ts in GROUPS for t in ts}
    tables, edges = {}, []
    for name in names:
        ddl = con.execute('SELECT sql FROM sqlite_master WHERE name=?', (name,)).fetchone()[0]
        source_ddl_match = re.search(
            r'CREATE (?:VIRTUAL )?TABLE IF NOT EXISTS\s+' + re.escape(name) + r'\b.*?\n\);',
            source,
            re.S,
        )
        source_ddl = source_ddl_match.group(0) if source_ddl_match else ''
        columns = []
        # 注释只作为说明，实际字段和约束均以 SQLite 解析结果为准。
        for _, col, typ, nn, default, pk in con.execute(f'PRAGMA table_info("{name}")'):
            line = re.search(r'^\s*' + re.escape(col) + r'\s+[^\n]*', ddl, re.M)
            source_line = re.search(r'^\s*' + re.escape(col) + r'(?:\s+[^\n,]*)?,?\s*--[^\n]*', source_ddl, re.M)
            comment_line = line if line and '--' in line.group(0) else source_line
            comment = comment_line.group(0).split('--', 1)[1].strip() if comment_line else ''
            columns.append(dict(name=col, type=typ or 'FTS', nn=bool(nn or (pk and typ == 'INTEGER')), default=default, pk=pk, comment=comment))
        indexes = []
        for _, idx, unique, origin, partial in con.execute(f'PRAGMA index_list("{name}")'):
            cols = [r[2] for r in con.execute(f'PRAGMA index_info("{idx}")')]
            sqlrow = con.execute('SELECT sql FROM sqlite_master WHERE name=?', (idx,)).fetchone()
            indexes.append(dict(name=idx, columns=cols, unique=bool(unique), origin=origin, partial=bool(partial), sql=sqlrow[0] if sqlrow else None))
        fks = {}
        for fid, seq, parent, child_col, parent_col, update, delete, match in con.execute(f'PRAGMA foreign_key_list("{name}")'):
            fk = fks.setdefault(fid, dict(child=name, parent=parent, cols=[], refs=[], delete=delete, update=update, logical=False))
            fk['cols'].append(child_col)
            fk['refs'].append(parent_col)
        for fk in fks.values():
            unique = any(i['unique'] and not i['partial'] and set(i['columns']).issubset(fk['cols']) for i in indexes)
            pk_cols = [c['name'] for c in columns if c['pk']]
            unique |= bool(pk_cols and set(pk_cols).issubset(fk['cols']))
            fk['child_many'] = '0..1' if unique else '0..*'
            fk['parent_many'] = '1' if all(next(c for c in columns if c['name'] == n)['nn'] for n in fk['cols']) else '0..1'
            fk['id'] = f'R{len(edges)+1:02d}'
            fk['label'] = ', '.join(fk['cols'])
            edges.append(fk)
        group, color = next((g, color) for g, color, ns in GROUPS if name in ns)
        tables[name] = dict(name=name, title=NAMES[name], columns=columns, indexes=indexes, ddl=ddl, group=group, color=color, virtual='VIRTUAL TABLE' in ddl,
                            triggers=[r[0] for r in con.execute("SELECT sql FROM sqlite_master WHERE type='trigger' AND tbl_name=?", (name,))])
    logical = [
        ('settings', 'sessions', 'current_session_id：字符串设置值，由 Rust 校验，无外键'),
        ('game_runs', 'game_plugins', 'plugin_id：历史身份快照；插件登记删除后游戏局仍保留'),
        ('currency_ledger', 'game_plugins', 'plugin_id：历史身份快照；不依赖当前插件登记'),
        ('currency_ledger', 'user_state', '同一事务更新余额与流水；无外键'),
        ('currency_ledger', 'accessories', 'ref_id：制作／卖出的历史饰品引用；饰品删除后账单保留'),
        ('long_memories_fts', 'long_memories', 'rowid ↔ id：外部内容 FTS5 索引，触发器同步，可重建'),
        ('long_memory_embeddings', 'embedding_providers', 'model_key：模型配置标识；不是 provider ID 外键'),
    ]
    for child, parent, label in logical:
        edges.append(dict(id=f'L{sum(e["logical"] for e in edges)+1:02d}', child=child, parent=parent, label=label, logical=True))
    assert not con.execute('PRAGMA foreign_key_check').fetchall()
    con.close()
    return dict(tables=tables, edges=edges, groups=[dict(name=g, color=c, tables=ns) for g, c, ns in GROUPS], sha256=hashlib.sha256(source.encode()).hexdigest())


def esc(value):
    return html.escape(str(value), quote=True)


def layout(data):
    positions = {}
    # 每个业务域一行；各行保留独立标题，长类框不挤压相邻业务域。
    y = 150
    bands = []
    for group in data['groups']:
        bands.append((y, group))
        y += 56
        for start in range(0, len(group['tables']), 4):
            row = group['tables'][start:start+4]
            heights = []
            for col, name in enumerate(row):
                t = data['tables'][name]
                relation_count = sum(e['child'] == name or e['parent'] == name for e in data['edges'])
                height = 88 + len(t['columns']) * 25 + max(1, (relation_count + 7) // 8) * 20 + 8
                positions[name] = dict(x=60+col*635, y=y, w=580, h=height)
                heights.append(height)
            y += max(heights) + 100
    return positions, bands, y


def make_svg(data):
    pos, bands, bottom = layout(data)
    edge_rows = []
    for e in data['edges']:
        if e['logical']:
            label = f'{e["id"]}  {e["child"]} → {e["parent"]} ｜逻辑关联｜{e["label"]}'
        else:
            label = f'{e["id"]}  {e["child"]} ({", ".join(e["cols"])}) → {e["parent"]} ({", ".join(e["refs"])}) ｜子 {e["child_many"]} : 父 {e["parent_many"]} ｜DELETE {e["delete"]}'
        edge_rows.append(label)
    width, height = 2640, bottom + 150 + len(edge_rows)*29
    s = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
         '<style>text{font-family:"Microsoft YaHei",Arial,sans-serif} .mono{font-family:Consolas,monospace} .edge{opacity:.28} .edge:hover{opacity:1;stroke-width:4} .card:hover rect{stroke-width:3}</style>',
         '<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10" fill="none" stroke="#64748b"/></marker></defs>',
         '<rect width="100%" height="100%" fill="#f5f7fb"/>',
         '<text x="60" y="65" font-size="34" fill="#172554" font-weight="700">颜喵 · 数据库完整类图</text>',
         '<text x="60" y="105" font-size="19" fill="#475569">data/yanwenzi.db · 30 张业务表 + 1 张 FTS5 虚拟表 · V1 设计蓝图 · 来源 db/schema.sql</text>']
    for y, g in bands:
        s.append(f'<text x="60" y="{y+24}" fill="{g["color"]}" font-size="25" font-weight="700">{esc(g["name"])}</text>')
    s.append('<g id="edges">')
    for i, e in enumerate(data['edges']):
        a,b = pos[e['child']], pos[e['parent']]
        x1,y1 = a['x']+a['w'], a['y']+42
        x2,y2 = b['x']+b['w'], b['y']+42
        lane = max(x1,x2)+15+(i%7)*5
        path = f'M{x1} {y1} H{lane} V{y2} H{x2}'
        if e['child']==e['parent']:
            path=f'M{x1} {y1} h35 v65 h-35'
        s.append(f'<path class="edge" data-child="{e["child"]}" data-parent="{e["parent"]}" d="{path}" fill="none" stroke="#64748b" stroke-width="2" marker-end="url(#arrow)" {"stroke-dasharray=\"7 5\"" if e["logical"] else ""}><title>{esc(edge_rows[i])}</title></path>')
    s.append('</g>')
    for name,t in data['tables'].items():
        p=pos[name]; x,y,w,h=p['x'],p['y'],p['w'],p['h']; c=t['color']
        related=[e['id'] for e in data['edges'] if e['child']==name or e['parent']==name]
        s += [f'<g class="card" id="table-{name}" data-table="{name}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" fill="white" stroke="{c}"/>',
              f'<path d="M{x} {y+78} H{x+w}" stroke="{c}"/>',
              f'<text x="{x+18}" y="{y+29}" font-size="20" font-weight="700" fill="{c}">{esc(t["title"])}{ " «FTS5»" if t["virtual"] else " «table»"}</text>',
              f'<text x="{x+18}" y="{y+57}" font-size="20" class="mono" fill="#0f172a">{name}</text>']
        fkcols={n for e in data['edges'] if not e['logical'] and e['child']==name for n in e['cols']}
        for j,col in enumerate(t['columns']):
            flags=('PK ' if col['pk'] else '')+('FK ' if col['name'] in fkcols else '')
            nullable='' if col['nn'] else '?'
            line=f'{flags}{col["name"]}: {col["type"]}{nullable}'
            if col['default'] is not None: line += f' = {col["default"]}'
            s.append(f'<text x="{x+18}" y="{y+103+j*25}" font-size="16" class="mono" fill="#334155">{esc(line)}<title>{esc(col["comment"])}</title></text>')
        chunks = [related[i:i+8] for i in range(0, len(related), 8)] or [[]]
        for j, chunk in enumerate(chunks):
            label = ('关系：' if j == 0 else '　　　') + (' / '.join(chunk) or '独立表')
            s.append(f'<text x="{x+18}" y="{y+h-13-(len(chunks)-1-j)*20}" font-size="13" fill="#64748b">{esc(label)}</text>')
        s.append('</g>')
    s.append(f'<text x="60" y="{bottom+12}" font-size="25" font-weight="700" fill="#172554">关联明细 · 箭头由引用方指向被引用方</text>')
    s.append(f'<text x="60" y="{bottom+47}" font-size="17" fill="#475569">实线 R = 数据库外键；虚线 L = 逻辑关联。? = SQLite 允许 NULL；PK/FK = 主键/外键。唯一约束、CHECK、索引见离线页面。</text>')
    for i,line in enumerate(edge_rows):
        s.append(f'<text x="60" y="{bottom+89+i*29}" font-size="16" fill="#334155">{esc(line)}</text>')
    s.append('</svg>')
    return '\n'.join(s), pos


def make_mermaid(data):
    lines=['classDiagram', 'direction LR']
    for name,t in data['tables'].items():
        lines += [f'class {name} {{', f'  <<{"FTS5" if t["virtual"] else "table"}>>']
        fkcols={n for e in data['edges'] if not e['logical'] and e['child']==name for n in e['cols']}
        for c in t['columns']:
            flags=(' PK' if c['pk'] else '')+(' FK' if c['name'] in fkcols else '')
            lines.append(f'  {c["type"] or "FTS"} {c["name"]}{flags}')
        lines.append('}')
    for e in data['edges']:
        if e['logical']:
            lines.append(f'{e["child"]} ..> {e["parent"]} : {e["id"]} logical')
        else:
            lines.append(f'{e["child"]} "{e["child_many"]}" --> "{e["parent_many"]}" {e["parent"]} : {e["id"]} {", ".join(e["cols"])} / {e["delete"]}')
    return '\n'.join(lines)+'\n'


def main():
    data=extract()
    svg, positions=make_svg(data)
    data['positions']=positions
    template=Path(__file__).with_name('database_diagram.template.html').read_text(encoding='utf-8')
    rendered=template.replace('__DATA__', json.dumps(data, ensure_ascii=False).replace('<','\\u003c')).replace('__SVG__',svg)
    (OUT/'数据库完整类图.html').write_text(rendered,encoding='utf-8')
    (OUT/'数据库完整类图.svg').write_text(svg,encoding='utf-8')
    (OUT/'数据库完整类图.mmd').write_text(make_mermaid(data),encoding='utf-8')
    print(json.dumps(dict(tables=len(data['tables']), fields=sum(len(t['columns']) for t in data['tables'].values()), foreign_keys=sum(not e['logical'] for e in data['edges']), logical_links=sum(e['logical'] for e in data['edges'])),ensure_ascii=False))


if __name__=='__main__':
    main()
