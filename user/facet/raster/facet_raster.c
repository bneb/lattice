/*
 * Facet Raster — Resolution-Independent Path Rasterizer (C Implementation)
 *
 * Algorithm-identical to raster.salt: adaptive de Casteljau flattening,
 * signed-area coverage accumulation, and scanline fill with winding rules.
 *
 * Build: clang -O3 -c facet_raster.c -o facet_raster.o
 */

#include "facet_raster.h"
#include <math.h>
#include <stdlib.h>
#include <string.h>

/* ═══════════════════════════════════════════════════════════════════════════
 * Path Construction
 * ═══════════════════════════════════════════════════════════════════════════
 */

FacetPath *facet_path_new(void) {
  FacetPath *p = (FacetPath *)calloc(1, sizeof(FacetPath));
  if (!p)
    return NULL;
  p->capacity = 64;
  p->cmds = (FacetPathCmd *)malloc(p->capacity * sizeof(FacetPathCmd));
  if (!p->cmds) {
    free(p);
    return NULL;
  }
  p->count = 0;
  return p;
}

static void path_ensure_capacity(FacetPath *p) {
  if (p->count >= p->capacity) {
    size_t new_capacity = p->capacity * 2;
    FacetPathCmd *new_cmds =
        (FacetPathCmd *)realloc(p->cmds, new_capacity * sizeof(FacetPathCmd));
    if (new_cmds) {
      p->cmds = new_cmds;
      p->capacity = new_capacity;
    }
  }
}

void facet_path_move_to(FacetPath *path, float x, float y) {
  path_ensure_capacity(path);
  FacetPathCmd *cmd = &path->cmds[path->count++];
  cmd->type = FACET_CMD_MOVE_TO;
  cmd->points[0] = (FacetPoint){x, y};
}

void facet_path_line_to(FacetPath *path, float x, float y) {
  path_ensure_capacity(path);
  FacetPathCmd *cmd = &path->cmds[path->count++];
  cmd->type = FACET_CMD_LINE_TO;
  cmd->points[0] = (FacetPoint){x, y};
}

void facet_path_quad_to(FacetPath *path, float cx, float cy, float x, float y) {
  path_ensure_capacity(path);
  FacetPathCmd *cmd = &path->cmds[path->count++];
  cmd->type = FACET_CMD_QUAD_TO;
  cmd->points[0] = (FacetPoint){cx, cy}; /* control */
  cmd->points[1] = (FacetPoint){x, y};   /* end */
}

void facet_path_cubic_to(FacetPath *path, float c1x, float c1y, float c2x,
                         float c2y, float x, float y) {
  path_ensure_capacity(path);
  FacetPathCmd *cmd = &path->cmds[path->count++];
  cmd->type = FACET_CMD_CUBIC_TO;
  cmd->points[0] = (FacetPoint){c1x, c1y}; /* ctrl1 */
  cmd->points[1] = (FacetPoint){c2x, c2y}; /* ctrl2 */
  cmd->points[2] = (FacetPoint){x, y};     /* end */
}

void facet_path_close(FacetPath *path) {
  path_ensure_capacity(path);
  FacetPathCmd *cmd = &path->cmds[path->count++];
  cmd->type = FACET_CMD_CLOSE;
}

void facet_path_free(FacetPath *path) {
  if (!path)
    return;
  free(path->cmds);
  free(path);
}

/* ═══════════════════════════════════════════════════════════════════════════
 * Canvas
 * ═══════════════════════════════════════════════════════════════════════════
 */

FacetCanvas *facet_canvas_new(uint32_t width, uint32_t height) {
  FacetCanvas *c = (FacetCanvas *)calloc(1, sizeof(FacetCanvas));
  if (!c)
    return NULL;
  c->width = width;
  c->height = height;
  c->stride = width * 4;
  c->pixels = (uint8_t *)calloc(c->stride * height, 1);
  if (!c->pixels) {
    free(c);
    return NULL;
  }
  return c;
}

void facet_canvas_clear(FacetCanvas *canvas, uint32_t rgba) {
  uint8_t r = (rgba >> 24) & 0xFF;
  uint8_t g = (rgba >> 16) & 0xFF;
  uint8_t b = (rgba >> 8) & 0xFF;
  uint8_t a = rgba & 0xFF;

  for (uint32_t y = 0; y < canvas->height; y++) {
    uint8_t *row = canvas->pixels + y * canvas->stride;
    for (uint32_t x = 0; x < canvas->width; x++) {
      row[x * 4 + 0] = r;
      row[x * 4 + 1] = g;
      row[x * 4 + 2] = b;
      row[x * 4 + 3] = a;
    }
  }
}

void facet_canvas_free(FacetCanvas *canvas) {
  if (!canvas)
    return;
  free(canvas->pixels);
  free(canvas);
}

/* ═══════════════════════════════════════════════════════════════════════════
 * Bézier Flattening (callback-based, exposed for testing)
 * ═══════════════════════════════════════════════════════════════════════════
 */

void facet_flatten_quad(FacetPoint p0, FacetPoint p1, FacetPoint p2,
                        float tolerance, FacetFlattenCallback callback,
                        void *user_data) {
  /* Flatness test: distance from control point to chord midpoint */
  float mx = (p0.x + p2.x) * 0.5f;
  float my = (p0.y + p2.y) * 0.5f;
  float dx = p1.x - mx;
  float dy = p1.y - my;
  float dist_sq = dx * dx + dy * dy;

  if (dist_sq <= tolerance * tolerance) {
    callback(p2.x, p2.y, user_data);
    return;
  }

  /* de Casteljau subdivision */
  FacetPoint p01 = {(p0.x + p1.x) * 0.5f, (p0.y + p1.y) * 0.5f};
  FacetPoint p12 = {(p1.x + p2.x) * 0.5f, (p1.y + p2.y) * 0.5f};
  FacetPoint mid = {(p01.x + p12.x) * 0.5f, (p01.y + p12.y) * 0.5f};

  facet_flatten_quad(p0, p01, mid, tolerance, callback, user_data);
  facet_flatten_quad(mid, p12, p2, tolerance, callback, user_data);
}

void facet_flatten_cubic(FacetPoint p0, FacetPoint p1, FacetPoint p2,
                         FacetPoint p3, float tolerance,
                         FacetFlattenCallback callback, void *user_data) {
  /* Flatness test: max distance of control points from chord */
  float ux = 3.0f * p1.x - 2.0f * p0.x - p3.x;
  float uy = 3.0f * p1.y - 2.0f * p0.y - p3.y;
  float vx = 3.0f * p2.x - p0.x - 2.0f * p3.x;
  float vy = 3.0f * p2.y - p0.y - 2.0f * p3.y;
  float ux2 = ux * ux, uy2 = uy * uy;
  float vx2 = vx * vx, vy2 = vy * vy;
  float max_x = ux2 > vx2 ? ux2 : vx2;
  float max_y = uy2 > vy2 ? uy2 : vy2;
  float flat_sq = max_x + max_y;

  if (flat_sq <= 16.0f * tolerance * tolerance) {
    callback(p3.x, p3.y, user_data);
    return;
  }

  /* de Casteljau subdivision */
  FacetPoint p01 = {(p0.x + p1.x) * 0.5f, (p0.y + p1.y) * 0.5f};
  FacetPoint p12 = {(p1.x + p2.x) * 0.5f, (p1.y + p2.y) * 0.5f};
  FacetPoint p23 = {(p2.x + p3.x) * 0.5f, (p2.y + p3.y) * 0.5f};
  FacetPoint p012 = {(p01.x + p12.x) * 0.5f, (p01.y + p12.y) * 0.5f};
  FacetPoint p123 = {(p12.x + p23.x) * 0.5f, (p12.y + p23.y) * 0.5f};
  FacetPoint pmid = {(p012.x + p123.x) * 0.5f, (p012.y + p123.y) * 0.5f};

  facet_flatten_cubic(p0, p01, p012, pmid, tolerance, callback, user_data);
  facet_flatten_cubic(pmid, p123, p23, p3, tolerance, callback, user_data);
}

/* ═══════════════════════════════════════════════════════════════════════════
 * Internal Edge Table (used by fill, not exposed in header)
 * ═══════════════════════════════════════════════════════════════════════════
 */

typedef struct {
  float x0, y0, x1, y1;
  float dir; /* +1 downward, -1 upward */
} Edge;

typedef struct {
  Edge *edges;
  size_t count;
  size_t capacity;
} EdgeTable;

static EdgeTable et_new(void) {
  EdgeTable et;
  et.capacity = 256;
  et.edges = (Edge *)malloc(et.capacity * sizeof(Edge));
  if (!et.edges) {
    et.capacity = 0;
  }
  et.count = 0;
  return et;
}

static void et_push(EdgeTable *et, Edge e) {
  if (et->count >= et->capacity) {
    size_t new_capacity = et->capacity == 0 ? 256 : et->capacity * 2;
    Edge *new_edges = (Edge *)realloc(et->edges, new_capacity * sizeof(Edge));
    if (!new_edges)
      return;
    et->edges = new_edges;
    et->capacity = new_capacity;
  }
  et->edges[et->count++] = e;
}

static void et_free(EdgeTable *et) { free(et->edges); }

static void add_line_edge(EdgeTable *et, float x0, float y0, float x1,
                          float y1) {
  if (fabsf(y1 - y0) < 0.001f)
    return;
  if (y0 < y1) {
    et_push(et, (Edge){x0, y0, x1, y1, 1.0f});
  } else {
    et_push(et, (Edge){x1, y1, x0, y0, -1.0f});
  }
}

/* Internal flatten — appends edges directly to EdgeTable */
static void flatten_quad_et(EdgeTable *et, FacetPoint p0, FacetPoint p1,
                            FacetPoint p2, float tol, int depth) {
  if (depth > 16) {
    add_line_edge(et, p0.x, p0.y, p2.x, p2.y);
    return;
  }
  float mx = (p0.x + p2.x) * 0.5f;
  float my = (p0.y + p2.y) * 0.5f;
  float dx = p1.x - mx;
  float dy = p1.y - my;
  if (dx * dx + dy * dy <= tol * tol) {
    add_line_edge(et, p0.x, p0.y, p2.x, p2.y);
    return;
  }
  FacetPoint p01 = {(p0.x + p1.x) * 0.5f, (p0.y + p1.y) * 0.5f};
  FacetPoint p12 = {(p1.x + p2.x) * 0.5f, (p1.y + p2.y) * 0.5f};
  FacetPoint mid = {(p01.x + p12.x) * 0.5f, (p01.y + p12.y) * 0.5f};
  flatten_quad_et(et, p0, p01, mid, tol, depth + 1);
  flatten_quad_et(et, mid, p12, p2, tol, depth + 1);
}

static void flatten_cubic_et(EdgeTable *et, FacetPoint p0, FacetPoint p1,
                             FacetPoint p2, FacetPoint p3, float tol,
                             int depth) {
  if (depth > 16) {
    add_line_edge(et, p0.x, p0.y, p3.x, p3.y);
    return;
  }
  float ux = 3.0f * p1.x - 2.0f * p0.x - p3.x;
  float uy = 3.0f * p1.y - 2.0f * p0.y - p3.y;
  float vx = 3.0f * p2.x - p0.x - 2.0f * p3.x;
  float vy = 3.0f * p2.y - p0.y - 2.0f * p3.y;
  float ux2 = ux * ux, uy2 = uy * uy;
  float vx2 = vx * vx, vy2 = vy * vy;
  float max_x = ux2 > vx2 ? ux2 : vx2;
  float max_y = uy2 > vy2 ? uy2 : vy2;
  if (max_x + max_y <= 16.0f * tol * tol) {
    add_line_edge(et, p0.x, p0.y, p3.x, p3.y);
    return;
  }
  FacetPoint p01 = {(p0.x + p1.x) * 0.5f, (p0.y + p1.y) * 0.5f};
  FacetPoint p12 = {(p1.x + p2.x) * 0.5f, (p1.y + p2.y) * 0.5f};
  FacetPoint p23 = {(p2.x + p3.x) * 0.5f, (p2.y + p3.y) * 0.5f};
  FacetPoint p012 = {(p01.x + p12.x) * 0.5f, (p01.y + p12.y) * 0.5f};
  FacetPoint p123 = {(p12.x + p23.x) * 0.5f, (p12.y + p23.y) * 0.5f};
  FacetPoint pmid = {(p012.x + p123.x) * 0.5f, (p012.y + p123.y) * 0.5f};
  flatten_cubic_et(et, p0, p01, p012, pmid, tol, depth + 1);
  flatten_cubic_et(et, pmid, p123, p23, p3, tol, depth + 1);
}

/* ═══════════════════════════════════════════════════════════════════════════
 * Canvas blend — Source Over compositing
 * ═══════════════════════════════════════════════════════════════════════════
 */

static void canvas_blend(FacetCanvas *c, int x, int y, uint8_t r, uint8_t g,
                         uint8_t b, uint8_t a) {
  if (x < 0 || x >= (int)c->width || y < 0 || y >= (int)c->height)
    return;
  uint8_t *px = c->pixels + (uint32_t)y * c->stride + (uint32_t)x * 4;
  if (a == 255) {
    px[0] = r;
    px[1] = g;
    px[2] = b;
    px[3] = a;
  } else if (a > 0) {
    int src_a = a;
    int inv_a = 255 - src_a;
    px[0] = (uint8_t)((int)r + ((int)px[0] * inv_a) / 255);
    px[1] = (uint8_t)((int)g + ((int)px[1] * inv_a) / 255);
    px[2] = (uint8_t)((int)b + ((int)px[2] * inv_a) / 255);
    px[3] = (uint8_t)(src_a + ((int)px[3] * inv_a) / 255);
  }
}

/* ═══════════════════════════════════════════════════════════════════════════
 * Fill — Signed-Area Scanline Rasterizer
 * ═══════════════════════════════════════════════════════════════════════════
 */

void facet_canvas_fill(FacetCanvas *canvas, const FacetPath *path,
                       uint32_t rgba, FacetWindingRule rule) {
  if (!canvas || !path || path->count == 0)
    return;

  uint8_t cr = (rgba >> 24) & 0xFF;
  uint8_t cg = (rgba >> 16) & 0xFF;
  uint8_t cb = (rgba >> 8) & 0xFF;
  uint8_t ca = rgba & 0xFF;

  const float tolerance = 0.25f;
  EdgeTable et = et_new();

  /* Phase 1: Flatten path commands → edge table */
  FacetPoint cur = {0, 0}, sub_start = {0, 0};
  for (size_t i = 0; i < path->count; i++) {
    const FacetPathCmd *cmd = &path->cmds[i];
    switch (cmd->type) {
    case FACET_CMD_MOVE_TO:
      cur = cmd->points[0];
      sub_start = cur;
      break;
    case FACET_CMD_LINE_TO:
      add_line_edge(&et, cur.x, cur.y, cmd->points[0].x, cmd->points[0].y);
      cur = cmd->points[0];
      break;
    case FACET_CMD_QUAD_TO:
      flatten_quad_et(&et, cur, cmd->points[0], cmd->points[1], tolerance, 0);
      cur = cmd->points[1];
      break;
    case FACET_CMD_CUBIC_TO:
      flatten_cubic_et(&et, cur, cmd->points[0], cmd->points[1], cmd->points[2],
                       tolerance, 0);
      cur = cmd->points[2];
      break;
    case FACET_CMD_CLOSE:
      add_line_edge(&et, cur.x, cur.y, sub_start.x, sub_start.y);
      cur = sub_start;
      break;
    }
  }

  if (et.count == 0) {
    et_free(&et);
    return;
  }

  /* Phase 2: Compute bounding box */
  float min_y = et.edges[0].y0;
  float max_y = et.edges[0].y1;
  for (size_t j = 1; j < et.count; j++) {
    if (et.edges[j].y0 < min_y)
      min_y = et.edges[j].y0;
    if (et.edges[j].y1 > max_y)
      max_y = et.edges[j].y1;
  }

  int y_start = (int)floorf(min_y);
  int y_end = (int)ceilf(max_y);
  if (y_start < 0)
    y_start = 0;
  if (y_end > (int)canvas->height)
    y_end = (int)canvas->height;

  /* Phase 3: Coverage buffer */
  float *coverage = (float *)calloc(canvas->width, sizeof(float));
  if (!coverage) {
    et_free(&et);
    return;
  }

  /* Phase 4: Scanline sweep */
  for (int scanline = y_start; scanline < y_end; scanline++) {
    float scan_y = (float)scanline + 0.5f;

    memset(coverage, 0, canvas->width * sizeof(float));

    /* Accumulate coverage from intersecting edges */
    for (size_t ei = 0; ei < et.count; ei++) {
      const Edge *e = &et.edges[ei];
      if (scan_y >= e->y0 && scan_y < e->y1) {
        float t = (scan_y - e->y0) / (e->y1 - e->y0);
        float x_int = e->x0 + t * (e->x1 - e->x0);
        int col = (int)floorf(x_int);
        if (col < 0)
          col = 0;
        if (col < (int)canvas->width) {
          coverage[col] += e->dir;
        }
      }
    }

    /* Integrate and emit */
    float winding = 0.0f;
    for (int px = 0; px < (int)canvas->width; px++) {
      winding += coverage[px];

      float fill_amount = 0.0f;
      float abs_w = fabsf(winding);
      if (rule == FACET_WINDING_NONZERO) {
        if (abs_w > 0.001f)
          fill_amount = 1.0f;
      } else {
        /* Even-odd */
        float w_floor = floorf(abs_w);
        float w_frac = abs_w - w_floor;
        float half = floorf(w_floor * 0.5f);
        int is_odd = (w_floor - half * 2.0f) > 0.5f;
        if (is_odd)
          fill_amount = 1.0f;
        else if (w_frac > 0.5f)
          fill_amount = 1.0f;
      }

      if (fill_amount > 0.001f) {
        uint8_t alpha = (uint8_t)(fill_amount * (float)ca);
        canvas_blend(canvas, px, scanline, cr, cg, cb, alpha);
      }
    }
  }

  free(coverage);
  et_free(&et);
}

void facet_canvas_fill_nonzero(FacetCanvas *canvas, const FacetPath *path,
                               uint32_t rgba) {
  facet_canvas_fill(canvas, path, rgba, FACET_WINDING_NONZERO);
}
