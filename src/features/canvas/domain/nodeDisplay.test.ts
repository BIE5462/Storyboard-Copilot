import { describe, expect, it } from 'vitest';

import { CANVAS_NODE_TYPES } from './canvasNodes';
import { isNodeUsingDefaultDisplayName, resolveNodeDisplayName } from './nodeDisplay';

const translate = (key: string) => `translated:${key}`;

describe('node display names', () => {
  it('translates empty and legacy default names', () => {
    expect(resolveNodeDisplayName(CANVAS_NODE_TYPES.upload, {}, translate)).toBe(
      'translated:node.defaults.upload'
    );
    expect(
      resolveNodeDisplayName(
        CANVAS_NODE_TYPES.upload,
        { displayName: '上传图片' },
        translate
      )
    ).toBe('translated:node.defaults.upload');
  });

  it('keeps user customized titles unchanged', () => {
    expect(
      resolveNodeDisplayName(
        CANVAS_NODE_TYPES.upload,
        { displayName: '我的中文标题' },
        translate
      )
    ).toBe('我的中文标题');
  });

  it('recognizes legacy default display names as default usage', () => {
    expect(
      isNodeUsingDefaultDisplayName(CANVAS_NODE_TYPES.upload, { displayName: '上传图片' })
    ).toBe(true);
    expect(
      isNodeUsingDefaultDisplayName(CANVAS_NODE_TYPES.upload, { displayName: '自定义上传' })
    ).toBe(false);
  });
});
