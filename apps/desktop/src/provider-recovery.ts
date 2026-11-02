/** Explain known recovery failures without treating an unverified write as applied. */
export function recoveryErrorText(value: unknown, ru: boolean): string {
  const record =
    value && typeof value === 'object' ? (value as { code?: unknown; message?: unknown }) : null;
  const text = typeof record?.message === 'string' ? record.message : String(value);
  if (record?.code === 'readback_mismatch' || /readback differs|readback mismatch/i.test(text))
    return ru
      ? 'Устройство вернуло другое значение. Запись не подтверждена; новые изменения остановлены. Проверьте сохранённый снимок перед восстановлением.'
      : 'The device returned a different value. The write is unconfirmed and further edits are stopped. Review the saved snapshot before recovery.';
  if (record?.code === 'write_uncertain' || /write.*uncertain|uncertain.*write/i.test(text))
    return ru
      ? 'Не удалось подтвердить результат записи. Новые изменения остановлены; сохранённый снимок остаётся доступным для явного восстановления.'
      : 'The write result could not be confirmed. Further edits are stopped; the saved snapshot remains available for explicit recovery.';
  if (record?.code === 'recovery_pending' || record?.code === 'recovery_required')
    return ru
      ? 'Предыдущее изменение ещё требует проверки и восстановления. Сначала просмотрите сохранённый снимок выбранного устройства.'
      : 'A previous change still needs review and recovery. Review the saved snapshot for the selected device first.';
  return text.slice(0, 500);
}
