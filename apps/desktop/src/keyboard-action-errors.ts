export function keyboardActionError(error: unknown, ru: boolean): string {
  const message = String(error ?? '');
  if (!ru) return message;
  if (message.includes('currently needs Static'))
    return 'Для временной подсветки выберите «Статичный», «Один цвет» и яркость от 2 во вкладке «Освещение». Затем вернитесь в «Действия». Текущий эффект не изменён.';
  if (message.includes('Finish manual lighting editing'))
    return 'Завершите ручное редактирование подсветки и вернитесь во вкладку «Действия», затем повторите проверку.';
  if (message.includes('more than one active device'))
    return 'Служебная привязка конфликтует с другим устройством. Назначение сохранено; проверьте привязки в настройках устройства.';
  if (
    message.includes('already registered') ||
    message.includes('occupied') ||
    message.includes('already in use')
  )
    return 'Служебная привязка занята. Назначение сохранено, но источник ввода пока недоступен.';
  if (message.includes('Global shortcut registration is unavailable'))
    return 'Прежний способ обработки служебной клавиши недоступен. Проверьте источник ввода в настройках устройства.';
  if (message.includes('disconnected or needs recovery'))
    return 'Устройство отключено или ожидает восстановления. Проверьте его состояние в настройках устройства.';
  if (message.includes('does not declare experimental temporary key lighting'))
    return 'Этот драйвер ещё не объявляет временную подсветку клавиш. Обновите драйвер устройства и сопоставьте подключённый экземпляр.';
  if (message.includes('original key colour'))
    return 'Драйвер не сообщил исходный цвет клавиши. Уведомление остановлено; проверьте состояние восстановления устройства.';
  return message;
}
