"""Pango/Cairo viewport layout for the application's immutable paragraph model.

Document offsets are Python codepoints; Pango indices are UTF-8 bytes. Only the
visible viewport is rasterized. Frame caret/selection coordinates retain the
frontend's historical 15-units-per-pixel geometry contract.
"""
from bisect import bisect_right
from dataclasses import dataclass
from io import BytesIO
import base64
import math
import ctypes

import gi
gi.require_version('Pango', '1.0')
gi.require_version('PangoCairo', '1.0')
from gi.repository import Pango, PangoCairo
import cairo
import regex

SCALE = Pango.SCALE
GEOMETRY_SCALE = 15
PADDING = 8


# Pango's installed GI annotation exposes n_ranges integers instead of the
# documented 2*n_ranges endpoints. Call the same public C API explicitly.
_pango = ctypes.CDLL('libpango-1.0.so.0')
_glib = ctypes.CDLL('libglib-2.0.so.0')
_x_ranges = _pango.pango_layout_line_get_x_ranges
_x_ranges.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.POINTER(ctypes.c_int)), ctypes.POINTER(ctypes.c_int)]
_glib.g_free.argtypes = [ctypes.c_void_p]

def line_x_ranges(line, start, end):
    ranges, count = ctypes.POINTER(ctypes.c_int)(), ctypes.c_int()
    _x_ranges(hash(line), start, end, ctypes.byref(ranges), ctypes.byref(count))
    try:
        return [ranges[index] for index in range(count.value * 2)]
    finally:
        _glib.g_free(ranges)


def byte_offsets(text):
    offsets, position, byte = [0], 0, 0
    for match in regex.finditer(r'[^\x00-\x7f]', text):
        index = match.start()
        if index > position:
            length = index - position
            offsets.extend(range(byte + 1, byte + length + 1))
            byte += length
        byte += len(match.group().encode('utf-8'))
        offsets.append(byte)
        position = index + 1
    offsets.extend(range(byte + 1, byte + len(text) - position + 1))
    return offsets


def codepoint_index(offsets, byte_index, trailing=0):
    return min(len(offsets) - 1, max(0, bisect_right(offsets, byte_index) - 1 + trailing))


def _node_runs(block):
    runs, position = [], 0
    for node in block.get('content', []):
        text = '\n' if node.get('type') == 'hardBreak' else node.get('text', '')
        marks = frozenset(mark.get('type') for mark in node.get('marks', []) if isinstance(mark, dict))
        if text and runs and runs[-1][2] == marks:
            runs[-1] = (runs[-1][0], position + len(text), marks)
        elif text:
            runs.append((position, position + len(text), marks))
        position += len(text)
    return runs


@dataclass
class Paragraph:
    key: tuple
    text: str
    offsets: list
    layout: object
    lines: list
    ink_bounds: list
    ink_top: float
    ink_bottom: float
    height: int
    y: float = 0


class NativeLayout:
    def __init__(self, model, width, fontFamily=None):
        if type(width) is not int or not 240 <= width <= 4096:
            raise ValueError('Invalid editor width.')
        self.model, self.width, self.font_family = model, width, fontFamily
        self._context = cairo.Context(cairo.ImageSurface(cairo.FORMAT_ARGB32, 1, 1))
        self._cache = []
        self.paragraphs = []
        self.lines = []
        self.layout_builds = 0
        self._preferred_x = None
        self._affinity = None
        self.total_height = 220
        self._sync()

    def prepare(self):
        """Shape changed paragraphs before the engine records processing time."""
        self._sync()

    def set_width(self, width):
        if type(width) is not int or not 240 <= width <= 4096:
            raise ValueError('Invalid editor width.')
        if width != self.width:
            self.width = width
            self.reset_vertical()
            self._affinity = None
        self._sync()

    def reflow(self, top, width):
        """Keep the viewport's logical passage anchored when its width changes.

        Soft wrapping can change the first character on a row. The old row's
        logical start remains in the new top row, with its vertical row offset
        retained. No manuscript or selection state is changed.
        """
        if type(top) is not int or not 0 <= top <= max(1_000_000, math.ceil(self.total_height + 2160)):
            raise ValueError('Invalid viewport.')
        if type(width) is not int or not 240 <= width <= 4096:
            raise ValueError('Invalid editor width.')
        self._sync()
        if width == self.width:
            return top
        if top == 0:
            self.set_width(width)
            return 0
        line_index = max(0, bisect_right([line[2] for line in self.lines], top) - 1)
        block, row, old_y, old_height = self.lines[line_index]
        paragraph = self.paragraphs[block]
        offset = codepoint_index(paragraph.offsets, paragraph.lines[row][0].start_index)
        within_row = max(0, min(top - old_y, old_height - 1))
        self.set_width(width)
        paragraph = self.paragraphs[block]
        new_row = paragraph.layout.index_to_line_x(paragraph.offsets[offset], False)[0]
        _, _, new_y, _, new_height, _ = paragraph.lines[new_row]
        return max(0, round(paragraph.y + new_y + min(within_row, new_height - 1)))

    def reset_vertical(self):
        self._preferred_x = None

    def reset_navigation(self):
        self.reset_vertical()
        self._affinity = None

    def _key(self, block, text, attrs):
        spacing = attrs.get('lineSpacing', 1.15)
        return (self.width, self.font_family, text, block.get('type'), attrs.get('lang', 'en'),
                attrs.get('dir', 'auto'), spacing, tuple(_node_runs(block)))

    def _build(self, block, text, attrs, key, previous=None):
        layout = PangoCairo.create_layout(self._context)
        context = layout.get_context()
        language = attrs.get('lang', 'en')
        context.set_language(Pango.Language.from_string(language if language != 'und' else 'en'))
        direction = attrs.get('dir', 'auto')
        context.set_base_dir(Pango.Direction.RTL if direction == 'rtl' else Pango.Direction.LTR)
        layout.set_auto_dir(direction == 'auto')
        layout.set_alignment(Pango.Alignment.RIGHT if direction == 'rtl' else Pango.Alignment.LEFT)
        layout.set_width((self.width - 2 * PADDING) * SCALE)
        layout.set_wrap(Pango.WrapMode.WORD_CHAR)
        layout.set_text(text, -1)
        heading = block.get('type') == 'heading'
        base_family = self.font_family or ('Vazirmatn,Noto Serif,Noto Serif CJK SC' if language == 'ar' else 'Noto Serif CJK SC,Noto Serif,Vazirmatn' if language.startswith('zh') else 'Noto Serif,Vazirmatn,Noto Serif CJK SC')
        base_size = 23 if language == 'ar' else 26 if heading else 18
        font = Pango.FontDescription()
        font.set_family(base_family)
        font.set_absolute_size(base_size * SCALE)
        layout.set_font_description(font)
        if previous is not None and text.startswith(previous.text):
            suffix = byte_offsets(text[len(previous.text):])
            offsets = previous.offsets + [previous.offsets[-1] + offset for offset in suffix[1:]]
        else:
            offsets = byte_offsets(text)
        attributes = Pango.AttrList()
        spacing = attrs.get('lineSpacing', 1.15)
        if type(spacing) not in (int, float) or not math.isfinite(spacing) or not 1 <= spacing <= 2:
            raise ValueError('Invalid paragraph line spacing.')
        # Font ascent/descent includes generous script fallback metrics. Apply
        # the requested leading to the displayed size rather than multiplying
        # that already enlarged box. Ink may extend outside a compact line box.
        attributes.insert(Pango.attr_line_height_new_absolute(round(base_size * spacing * SCALE)))
        # Let Pango choose script fonts from the ordered fallback family list.
        # A font boundary per script character would create thousands of attrs
        # in a mixed paragraph and can unnecessarily split shaping clusters.
        for start, end, marks in _node_runs(block):
            if marks:
                self._font_attribute(attributes, offsets, start, end, language, marks, heading)
        layout.set_attributes(attributes)
        lines, ink_bounds = [], []
        iterator = layout.get_iter()
        index = 0
        while True:
            ink, logical = iterator.get_line_extents()
            line = layout.get_line_readonly(index)
            lines.append((line, logical.x / SCALE, logical.y / SCALE, logical.width / SCALE,
                          logical.height / SCALE, iterator.get_baseline() / SCALE))
            ink_bounds.append((ink.y / SCALE, ink.height / SCALE))
            index += 1
            if not iterator.next_line():
                break
        _, height = layout.get_pixel_size()
        self.layout_builds += 1
        return Paragraph(key, text, offsets, layout, lines, ink_bounds,
                         min(y for y, _ in ink_bounds), max(y + h for y, h in ink_bounds),
                         max(1, height))

    def _font_attribute(self, attributes, offsets, start, end, script, marks, heading):
        family = self.font_family or ('Vazirmatn,Noto Serif,Noto Serif CJK SC' if script == 'ar' else 'Noto Serif CJK SC,Noto Serif,Vazirmatn' if script.startswith('zh') else 'Noto Serif,Vazirmatn,Noto Serif CJK SC')
        size = 23 if script == 'ar' else 26 if heading else 18
        font = Pango.FontDescription()
        font.set_family(family)
        font.set_absolute_size(size * SCALE)
        font.set_weight(Pango.Weight.BOLD if 'bold' in marks else Pango.Weight.NORMAL)
        font.set_style(Pango.Style.ITALIC if 'italic' in marks else Pango.Style.NORMAL)
        attr = Pango.attr_font_desc_new(font)
        attr.start_index, attr.end_index = offsets[start], offsets[end]
        attributes.insert(attr)
        if 'underline' in marks:
            attr = Pango.attr_underline_new(Pango.Underline.SINGLE)
            attr.start_index, attr.end_index = offsets[start], offsets[end]
            attributes.insert(attr)

    def _sync(self):
        paragraphs, flat_lines, y = [], [], PADDING
        old_by_identity = {id(block): paragraph for block, paragraph in getattr(self, "_block_cache", [])}
        block_cache = []
        for index, block in enumerate(self.model.blocks):
            text, attrs = self.model.block_text(index), self.model.block_attrs(index)
            key = self._key(block, text, attrs)
            cached = old_by_identity.get(id(block))
            if cached is None:
                cached = self._cache[index] if index < len(self._cache) else None
            paragraph = cached if cached and cached.key == key else self._build(block, text, attrs, key, cached)
            if index == 0:
                # Keep the initial baseline fixed while combining marks are
                # typed; reserve a font-size-based guard rather than moving
                # the document according to its current glyph ink.
                font_size = paragraph.layout.get_font_description().get_size() / SCALE
                y = PADDING + math.ceil(font_size / 2)
            paragraph.y = y
            paragraphs.append(paragraph)
            block_cache.append((block, paragraph))
            for line_index, line in enumerate(paragraph.lines):
                flat_lines.append((index, line_index, y + line[2], line[4]))
            y += paragraph.height
        self._cache = paragraphs
        self._block_cache = block_cache
        self.paragraphs, self.lines = paragraphs, flat_lines
        ink_bottom = max((paragraph.y + paragraph.ink_bottom for paragraph in paragraphs), default=y)
        self.total_height = max(220, math.ceil(max(y, ink_bottom) + PADDING))

    def _position(self, cursor):
        self._sync()
        block = max(0, min(len(self.paragraphs) - 1, int(cursor[0])))
        paragraph = self.paragraphs[block]
        offset = max(0, min(len(paragraph.text), int(cursor[1])))
        return block, offset, paragraph

    def cursor_rectangle(self, cursor=None):
        cursor = self.model.cursor if cursor is None else cursor
        block, offset, paragraph = self._position(cursor)
        affinity = self._affinity
        if affinity and affinity['cursor'] == (block, offset) and affinity['key'] == paragraph.key:
            strong, _ = paragraph.layout.get_cursor_pos(paragraph.offsets[offset])
            natural_line = paragraph.layout.index_to_line_x(paragraph.offsets[offset], False)[0]
            line_y = paragraph.lines[affinity['line']][2]
            natural_y = paragraph.lines[natural_line][2]
            return [PADDING + affinity['x'], paragraph.y + strong.y / SCALE + line_y - natural_y, 1, max(1, strong.height / SCALE)]
        strong, _ = paragraph.layout.get_cursor_pos(paragraph.offsets[offset])
        return [PADDING + strong.x / SCALE, paragraph.y + strong.y / SCALE, 1, max(1, strong.height / SCALE)]

    def locate_point(self, x, y):
        self._sync()
        index = max(0, bisect_right([paragraph.y for paragraph in self.paragraphs], y) - 1)
        paragraph = self.paragraphs[index]
        _, byte_index, trailing = paragraph.layout.xy_to_index(round((x - PADDING) * SCALE), round((y - paragraph.y) * SCALE))
        offset = codepoint_index(paragraph.offsets, byte_index, trailing)
        self.reset_navigation()
        return index, offset

    def _line_number(self, block, offset, paragraph):
        affinity = self._affinity
        if affinity and affinity['cursor'] == (block, offset) and affinity['key'] == paragraph.key:
            return affinity['line']
        return paragraph.layout.index_to_line_x(paragraph.offsets[offset], False)[0]

    def _set_line_affinity(self, block, offset, paragraph, line_index, end=False):
        line = paragraph.lines[line_index][0]
        start = codepoint_index(paragraph.offsets, line.start_index)
        finish = codepoint_index(paragraph.offsets, line.start_index + line.length)
        while finish > start and paragraph.text[finish - 1] == '\n':
            finish -= 1
        if end and finish > start:
            # Only the current visual line needs a grapheme search.
            cluster = start
            for match in regex.finditer(r'\X', paragraph.text[start:finish]):
                cluster = start + match.start()
            byte_index, trailing = paragraph.offsets[cluster], finish - cluster
            _, x = paragraph.layout.index_to_line_x(byte_index, True)
        else:
            byte_index, trailing = paragraph.offsets[offset], 0
            _, x = paragraph.layout.index_to_line_x(byte_index, False)
        self._affinity = {'cursor': (block, offset), 'key': paragraph.key, 'line': line_index,
                          'x': x / SCALE, 'index': byte_index, 'trailing': trailing}

    def line_edge(self, cursor, end):
        block, offset, paragraph = self._position(cursor)
        line_index = self._line_number(block, offset, paragraph)
        line = paragraph.lines[line_index][0]
        target = codepoint_index(paragraph.offsets, line.start_index + (line.length if end else 0))
        if end:
            while target > 0 and paragraph.text[target - 1] == '\n':
                target -= 1
        self.reset_vertical()
        self._set_line_affinity(block, target, paragraph, line_index, end)
        return block, target

    def visual_target(self, cursor, direction, ctrl=False):
        block, offset, paragraph = self._position(cursor)
        if ctrl and hasattr(self.model, 'word_target'):
            self.reset_navigation()
            return self.model.word_target(cursor, direction)
        affinity = self._affinity
        valid_affinity = affinity and affinity['cursor'] == (block, offset) and affinity['key'] == paragraph.key
        old_index = affinity['index'] if valid_affinity else paragraph.offsets[offset]
        old_trailing = affinity['trailing'] if valid_affinity else 0
        byte_index, trailing = paragraph.layout.move_cursor_visually(True, old_index, old_trailing, 1 if direction > 0 else -1)
        self.reset_navigation()
        if byte_index < 0:
            return (block - 1, len(self.paragraphs[block - 1].text)) if block else (0, 0)
        if byte_index > paragraph.offsets[-1]:
            return (block + 1, 0) if block + 1 < len(self.paragraphs) else (block, len(paragraph.text))
        return block, codepoint_index(paragraph.offsets, byte_index, trailing)

    def vertical_target(self, cursor, delta):
        block, offset, paragraph = self._position(cursor)
        line_index = self._line_number(block, offset, paragraph)
        global_line = next(index for index, entry in enumerate(self.lines) if entry[0] == block and entry[1] == line_index)
        if self._preferred_x is None:
            self._preferred_x = self.cursor_rectangle(cursor)[0]
        target = self.lines[max(0, min(len(self.lines) - 1, global_line + delta))]
        target_block, target_line, top, height = target
        target_paragraph = self.paragraphs[target_block]
        _, byte_index, trailing = target_paragraph.layout.xy_to_index(round((self._preferred_x - PADDING) * SCALE), round((top - target_paragraph.y + height / 2) * SCALE))
        target_offset = codepoint_index(target_paragraph.offsets, byte_index, trailing)
        line = target_paragraph.lines[target_line][0]
        line_end = codepoint_index(target_paragraph.offsets, line.start_index + line.length)
        self._set_line_affinity(target_block, target_offset, target_paragraph, target_line, target_offset == line_end)
        return target_block, target_offset

    def selection_rectangles(self, top, height):
        anchor = getattr(self.model, 'anchor', self.model.cursor)
        if anchor is None or anchor == self.model.cursor:
            return []
        first, last = sorted((tuple(anchor), tuple(self.model.cursor)))
        rectangles = []
        for block_index in range(first[0], last[0] + 1):
            paragraph = self.paragraphs[block_index]
            start = first[1] if block_index == first[0] else 0
            end = last[1] if block_index == last[0] else len(paragraph.text)
            for line, _, line_y, _, line_height, _ in paragraph.lines:
                y = paragraph.y + line_y
                if y + line_height < top or y > top + height:
                    continue
                ranges = line_x_ranges(line, paragraph.offsets[start], paragraph.offsets[end])
                for index in range(0, len(ranges), 2):
                    left, right = sorted((ranges[index] / SCALE, ranges[index + 1] / SCALE))
                    if right > left:
                        rectangles.append([PADDING + left, y, right - left, line_height])
            if block_index < last[0]:
                _, _, line_y, _, line_height, _ = paragraph.lines[-1]
                y = paragraph.y + line_y
                if y + line_height >= top and y <= top + height:
                    rectangles.append([self.width - PADDING - 4, y, 4, line_height])
        return [[round(value * GEOMETRY_SCALE) for value in rectangle] for rectangle in rectangles]

    def frame(self, top=0, height=680, followCursor=False):
        if type(height) is not int or not 120 <= height <= 2160 or type(followCursor) is not bool:
            raise ValueError('Invalid viewport.')
        self._sync()
        if type(top) is not int or not 0 <= top <= max(1_000_000, math.ceil(self.total_height + height)):
            raise ValueError('Invalid viewport.')
        caret = self.cursor_rectangle()
        if followCursor:
            block, offset = self.model.cursor
            paragraph = self.paragraphs[block]
            row = self._line_number(block, offset, paragraph)
            ink_y, ink_height = paragraph.ink_bounds[row]
            caret_top = min(caret[1], paragraph.y + ink_y)
            caret_bottom = max(caret[1] + caret[3], paragraph.y + ink_y + ink_height)
            if caret_top < top:
                top = max(0, math.floor(caret_top))
                if self.model.cursor == (0, 0):
                    top = 0
            elif caret_bottom > top + height:
                top = max(0, math.ceil(caret_bottom - height))
        top = min(top, max(0, self.total_height - height))
        surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, self.width, height)
        context = cairo.Context(surface)
        context.set_source_rgb(1, 254 / 255, 249 / 255)
        context.paint()
        context.set_source_rgb(62 / 255, 73 / 255, 64 / 255)
        self.rendered_lines = 0
        for paragraph in self.paragraphs:
            paint_top = paragraph.y + paragraph.ink_top
            paint_bottom = paragraph.y + paragraph.ink_bottom
            if paint_bottom < top or paint_top > top + height:
                continue
            for (line, line_x, line_y, _, line_height, baseline), (ink_y, ink_height) in zip(paragraph.lines, paragraph.ink_bounds):
                if paragraph.y + ink_y + ink_height < top or paragraph.y + ink_y > top + height:
                    continue
                context.move_to(PADDING + line_x, paragraph.y + baseline - top)
                PangoCairo.show_layout_line(context, line)
                self.rendered_lines += 1
        output = BytesIO()
        surface.write_to_png(output)
        return {'image': 'data:image/png;base64,' + base64.b64encode(output.getvalue()).decode('ascii'),
                'width': self.width, 'height': height, 'totalHeight': self.total_height, 'top': top,
                'origin': [0, 0], 'cursor': [round(value * GEOMETRY_SCALE) for value in caret],
                'selection': self.selection_rectangles(top, height), 'selectedText': self.model.selection_text(),
                'states': self.model.states() if hasattr(self.model, 'states') else {},
                'missingGlyphs': sum(paragraph.layout.get_unknown_glyphs_count() for paragraph in self.paragraphs)}
