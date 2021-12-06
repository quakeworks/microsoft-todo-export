import glob
import os

import commonmark as commonmark
import html2text


def get_baseline_name(fn):
    return os.path.splitext(fn)[0] + '.md'

html_files = glob.glob("content/*.html")
for fn in html_files:
    new_name = get_baseline_name(fn)
    h = html2text.HTML2Text()

    input = open(fn, 'r').read()
    actual = h.handle(input)

    f = open("out/" + new_name, "w")
    f.write(actual)
    f.close()

#
# def html_to_markdown(data):
#     markdown = converters.Html2Markdown().convert(data)
#     return markdown
