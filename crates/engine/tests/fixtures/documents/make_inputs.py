#!/usr/bin/env python3
"""Create small authored Office/PDF inputs. No application/helper dependency."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
P = 'http://schemas.openxmlformats.org/presentationml/2006/main'
A = 'http://schemas.openxmlformats.org/drawingml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
REL = 'http://schemas.openxmlformats.org/package/2006/relationships'
S = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'

def package(path, parts, types):
    defaults = '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>'
    overrides = ''.join(f'<Override PartName="/{name}" ContentType="{kind}"/>' for name, kind in types.items())
    parts = {'[Content_Types].xml': f'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">{defaults}{overrides}</Types>', **parts}
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
        for name, text in parts.items():
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, text.encode())

def pdf(path, streams):
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'']
    objects += [b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>', b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>']
    pages = []
    for stream in streams:
        page = len(objects) + 1
        pages.append(page)
        objects.append(f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {page+1} 0 R >>'.encode())
        raw = stream.encode()
        objects.append(f'<< /Length {len(raw)} >>\nstream\n'.encode() + raw + b'\nendstream')
    objects[1] = f'<< /Type /Pages /Kids [{" ".join(f"{p} 0 R" for p in pages)}] /Count {len(pages)} >>'.encode()
    result = bytearray(b'%PDF-1.4\n')
    offsets = []
    for i, obj in enumerate(objects, 1):
        offsets.append(len(result))
        result.extend(f'{i} 0 obj\n'.encode() + obj + b'\nendobj\n')
    xref = len(result)
    result.extend(f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode())
    for offset in offsets:
        result.extend(f'{offset:010} 00000 n \n'.encode())
    result.extend(f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    path.write_bytes(result)

def generate(root):
    root.mkdir(parents=True, exist_ok=True)
    para = lambda text: f'<w:p><w:r><w:t>{text}</w:t></w:r></w:p>'
    cell = lambda text: f'<w:tc><w:tcPr/>{para(text)}</w:tc>'
    document = f'''<w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Office field notes</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">A </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>qualified</w:t></w:r><w:r><w:t xml:space="preserve"> observation is </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>not a guarantee</w:t></w:r><w:r><w:t>.</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>First supplied item</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Nested supplied item</w:t></w:r></w:p>
<w:p><w:hyperlink r:id="guide"><w:r><w:t>Source guide</w:t></w:r></w:hyperlink></w:p>
<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/><w:gridCol w:w="2400"/></w:tblGrid><w:tr>{cell('Measurement')}{cell('Value')}</w:tr><w:tr>{cell('Count')}{cell('0')}</w:tr><w:tr>{cell('Absent')}{cell('')}</w:tr></w:tbl>
{para('Only the supplied input was inspected.')}<w:sectPr/></w:body></w:document>'''
    styles = f'<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style></w:styles>'
    numbering = f'<w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="3"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>'
    package(root/'field-notes.docx', {
        '_rels/.rels': f'<Relationships xmlns="{REL}"><Relationship Id="doc" Type="{R}/officeDocument" Target="word/document.xml"/></Relationships>',
        'word/document.xml': document, 'word/styles.xml': styles, 'word/numbering.xml': numbering,
        'word/_rels/document.xml.rels': f'<Relationships xmlns="{REL}"><Relationship Id="styles" Type="{R}/styles" Target="styles.xml"/><Relationship Id="numbering" Type="{R}/numbering" Target="numbering.xml"/><Relationship Id="guide" Type="{R}/hyperlink" Target="https://example.test/guide" TargetMode="External"/></Relationships>',
    }, {'word/document.xml': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml'})
    slide = lambda title, body: f'''<p:sld xmlns:p="{P}" xmlns:a="{A}"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{title}</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Body"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{body}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>'''
    package(root/'field-notes.pptx', {
        '_rels/.rels': f'<Relationships xmlns="{REL}"><Relationship Id="presentation" Type="{R}/officeDocument" Target="ppt/presentation.xml"/></Relationships>',
        'ppt/presentation.xml': f'<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="slide1"/><p:sldId id="257" r:id="slide2"/></p:sldIdLst></p:presentation>',
        'ppt/_rels/presentation.xml.rels': f'<Relationships xmlns="{REL}"><Relationship Id="slide1" Type="{R}/slide" Target="slides/slide1.xml"/><Relationship Id="slide2" Type="{R}/slide" Target="slides/slide2.xml"/></Relationships>',
        'ppt/slides/slide1.xml': slide('Slide one', 'A zero value is 0, not missing.'),
        'ppt/slides/slide2.xml': slide('Slide two', 'Only this supplied input was inspected.'),
        'ppt/slides/_rels/slide2.xml.rels': f'<Relationships xmlns="{REL}"><Relationship Id="notes" Type="{R}/notesSlide" Target="../notesSlides/notesSlide1.xml"/></Relationships>',
        'ppt/notesSlides/notesSlide1.xml': f'<p:notes xmlns:p="{P}" xmlns:a="{A}"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="Notes"/><p:cNvSpPr/><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>Speaker note: do not generalize.</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>',
    }, {'ppt/presentation.xml':'application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml', 'ppt/slides/slide1.xml':'application/vnd.openxmlformats-officedocument.presentationml.slide+xml', 'ppt/slides/slide2.xml':'application/vnd.openxmlformats-officedocument.presentationml.slide+xml'})
    package(root/'field-notes.xlsx', {
        '_rels/.rels': f'<Relationships xmlns="{REL}"><Relationship Id="workbook" Type="{R}/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        'xl/workbook.xml': f'<workbook xmlns="{S}" xmlns:r="{R}"><sheets><sheet name="Actuals" sheetId="1" r:id="sheet1"/><sheet name="Private notes" sheetId="2" r:id="sheet2" state="hidden"/></sheets></workbook>',
        'xl/_rels/workbook.xml.rels': f'<Relationships xmlns="{REL}"><Relationship Id="sheet1" Type="{R}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="sheet2" Type="{R}/worksheet" Target="worksheets/sheet2.xml"/></Relationships>',
        'xl/worksheets/sheet1.xml': f'<worksheet xmlns="{S}"><dimension ref="A1:D2"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Count</t></is></c><c r="B1" t="inlineStr"><is><t>Absent</t></is></c><c r="C1" t="inlineStr"><is><t>Formula cached value</t></is></c><c r="D1" t="inlineStr"><is><t>Literal formula text</t></is></c></row><row r="2"><c r="A2"><v>0</v></c><c r="B2"/><c r="C2"><f>SUM(A2,42)</f><v>42</v></c><c r="D2" t="inlineStr"><is><t>=SUM(A2,99)</t></is></c></row></sheetData></worksheet>',
        'xl/worksheets/sheet2.xml': f'<worksheet xmlns="{S}"><dimension ref="A1:A1"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Hidden note remains identified as hidden.</t></is></c></row></sheetData></worksheet>',
    }, {'xl/workbook.xml':'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml'})
    pdf(root/'native-text.pdf', ['BT /F2 18 Tf 40 740 Td (Native field notes) Tj ET\nBT /F1 12 Tf 40 710 Td (A zero value is 0, not missing.) Tj 0 -20 Td (Only this supplied input was inspected.) Tj ET', 'BT /F1 12 Tf 40 740 Td (Page two keeps the supplied text.) Tj ET'])
    pdf(root/'native-columns.pdf', ['BT /F2 18 Tf 40 740 Td (Column field notes) Tj ET\n' + '\n'.join(f'BT /F1 12 Tf {x} {y} Td ({text}) Tj ET' for x,y,text in [(40,700,'Left paragraph begins.'),(320,700,'Right paragraph begins.'),(40,680,'Left paragraph continues.'),(320,680,'Right paragraph continues.'),(40,660,'Left paragraph ends.'),(320,660,'Right paragraph ends.')])])
    pdf(root/'native-partial.pdf', ['BT /F1 12 Tf 40 740 Td (Readable first page.) Tj ET', ''])
    for name in ['commonmark.md','fidelity.html']:
        (root/name).write_bytes((Path(__file__).parent/name).read_bytes())
    manifest = []
    for path in sorted(root.iterdir()):
        if path.is_file() and path.name != 'manifest.json':
            raw = path.read_bytes()
            manifest.append({'name':path.name,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()})
    (root/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    generate(parser.parse_args().output)
