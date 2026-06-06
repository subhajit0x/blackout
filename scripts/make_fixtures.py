#!/usr/bin/env python3
"""Generate small files that DO carry metadata, so we can prove BLACKOUT removes it.
Pure stdlib — no Pillow/reportlab needed."""
import os, struct, zlib, zipfile, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "fixtures"
os.makedirs(OUT, exist_ok=True)

def p(name): return os.path.join(OUT, name)

# ---------- PNG with a tEXt comment + tIME ----------
def png_chunk(typ, data):
    return struct.pack(">I", len(data)) + typ + data + struct.pack(">I", zlib.crc32(typ + data) & 0xffffffff)

sig = b"\x89PNG\r\n\x1a\n"
# 1x1 transparent-ish image
ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)  # 8-bit RGB
raw = b"\x00\xff\x00\x00"  # one filtered scanline (filter byte + RGB)
idat = zlib.compress(raw)
text = b"Author\x00Subhajit (secret)"          # tEXt keyword\0value
tim = struct.pack(">HBBBBB", 2024, 1, 22, 7, 21, 0)  # tIME
with open(p("photo.png"), "wb") as f:
    f.write(sig)
    f.write(png_chunk(b"IHDR", ihdr))
    f.write(png_chunk(b"tEXt", text))
    f.write(png_chunk(b"tIME", tim))
    f.write(png_chunk(b"IDAT", idat))
    f.write(png_chunk(b"IEND", b""))

# ---------- WAV with a LIST/INFO tag ----------
def wav():
    fmt = struct.pack("<HHIIHH", 1, 1, 8000, 8000, 1, 8)  # PCM mono 8k 8-bit
    data = b"\x80" * 16
    info = b"INFOIART" + struct.pack("<I", 8) + b"SecretDJ"  # artist tag
    list_chunk = b"LIST" + struct.pack("<I", len(info)) + info
    body = b"WAVE"
    body += b"fmt " + struct.pack("<I", len(fmt)) + fmt
    body += list_chunk
    body += b"data" + struct.pack("<I", len(data)) + data
    with open(p("clip.wav"), "wb") as f:
        f.write(b"RIFF" + struct.pack("<I", len(body)) + body)
wav()

# ---------- MP3 with ID3v2 + ID3v1 ----------
def synchsafe(n):
    return bytes([(n >> 21) & 0x7f, (n >> 14) & 0x7f, (n >> 7) & 0x7f, n & 0x7f])

def mp3():
    # one TIT2 (title) frame
    frame_body = b"\x00" + b"My Secret Song"
    frame = b"TIT2" + struct.pack(">I", len(frame_body)) + b"\x00\x00" + frame_body
    tag_body = frame
    id3v2 = b"ID3" + b"\x04\x00" + b"\x00" + synchsafe(len(tag_body)) + tag_body
    # a couple of fake MPEG frames (0xFF 0xFB sync) so there's "audio"
    audio = b"\xff\xfb\x90\x00" + b"\x00" * 100
    id3v1 = b"TAG" + b"Title".ljust(30, b"\x00") + b"Artist".ljust(30, b"\x00") \
            + b"Album".ljust(30, b"\x00") + b"2024" + b"Comment".ljust(30, b"\x00") + b"\x00"
    assert len(id3v1) == 128, len(id3v1)
    with open(p("song.mp3"), "wb") as f:
        f.write(id3v2 + audio + id3v1)
mp3()

# ---------- DOCX with author/company metadata ----------
def docx():
    core = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>Subhajit Secret</dc:creator><cp:lastModifiedBy>Subhajit Secret</cp:lastModifiedBy><dcterms:created xsi:type="dcterms:W3CDTF">2024-01-22T07:21:00Z</dcterms:created></cp:coreProperties>'''
    app = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Company>ACME Secret Corp</Company><Manager>The Boss</Manager></Properties>'''
    ct = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'''
    doc = '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>'
    rels = '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'
    with zipfile.ZipFile(p("report.docx"), "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", ct)
        z.writestr("_rels/.rels", rels)
        z.writestr("docProps/core.xml", core)
        z.writestr("docProps/app.xml", app)
        z.writestr("word/document.xml", doc)
docx()

# ---------- PDF with an /Info dictionary ----------
def pdf():
    objs = []
    objs.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    objs.append(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    objs.append(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>")
    objs.append(b"<< /Author (Subhajit Secret) /Producer (SecretWriter 1.0) /Creator (SecretApp) /Title (Confidential) >>")
    body = b"%PDF-1.4\n"
    offsets = []
    for i, o in enumerate(objs, start=1):
        offsets.append(len(body))
        body += ("%d 0 obj\n" % i).encode() + o + b"\nendobj\n"
    xref_pos = len(body)
    body += b"xref\n0 %d\n" % (len(objs) + 1)
    body += b"0000000000 65535 f \n"
    for off in offsets:
        body += ("%010d 00000 n \n" % off).encode()
    body += b"trailer\n<< /Size %d /Root 1 0 R /Info 4 0 R >>\nstartxref\n%d\n%%%%EOF" % (len(objs) + 1, xref_pos)
    with open(p("doc.pdf"), "wb") as f:
        f.write(body)
pdf()

print("fixtures written to", OUT)
for n in sorted(os.listdir(OUT)):
    print("  ", n, os.path.getsize(p(n)), "bytes")
