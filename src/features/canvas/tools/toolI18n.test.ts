import { describe, expect, it } from 'vitest';

import { cropToolPlugin } from './builtInTools';

describe('tool i18n metadata', () => {
  it('keeps legacy labels while exposing translation keys', () => {
    expect(cropToolPlugin.label).toBe('裁剪');
    expect(cropToolPlugin.labelKey).toBe('tool.crop');

    const ratioField = cropToolPlugin.fields.find((field) => field.key === 'aspectRatio');
    expect(ratioField?.label).toBe('目标比例');
    expect(ratioField?.labelKey).toBe('toolEditor.crop.aspectRatio');
    expect(ratioField?.type).toBe('select');

    if (ratioField?.type !== 'select') {
      throw new Error('ratio field should be select');
    }

    expect(ratioField.options[0]).toMatchObject({
      label: '自由',
      labelKey: 'toolEditor.crop.ratio.free',
      value: 'free',
    });
  });
});
