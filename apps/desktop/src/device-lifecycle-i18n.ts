import type { Locale } from './types';
const en = {
  active: 'Your devices',
  saved: 'Saved / Removed',
  create: 'Create virtual device',
  remove: 'Remove from DeckWeave…',
  restore: 'Restore saved device',
  forget: 'Forget saved settings…',
  removeTitle: 'Remove this device?',
  forgetTitle: 'Permanently forget this device?',
  removeHint:
    'Its editor will close and running actions will stop. Profiles and settings will be saved here for restoration. Physical hardware is not reset.',
  forgetHint:
    'All saved profiles, actions and device settings for this instance will be permanently deleted. Other devices are unaffected. Physical hardware will stay excluded from automatic discovery.',
  cancel: 'Cancel',
  confirmRemove: 'Remove device',
  confirmForget: 'Forget settings',
  emptyActive:
    'No active devices. Restore a saved device, create a virtual device, or connect supported hardware.',
  emptySaved: 'Removed devices appear here with their profiles and settings.',
  autoDiscovery:
    'Mirabox appears automatically. Add hardware from installed device support through Discovered hardware. Removed devices require explicit restoration.',
  retained: 'Profiles and settings retained',
  missing: 'Device support is unavailable in this build',
  readd: 'Allow discovery again',
  forgotten: 'Forgotten hardware',
  forgottenHint:
    'Its settings were erased. Allowing discovery creates a new instance when this hardware is connected.',
  profiles: 'profiles',
  restoring: 'Physical devices restore offline until connected.',
  addSupport: 'For new models, install the corresponding device support package.',
} as const;
const ru: Record<keyof typeof en, string> = {
  active: 'Ваши устройства',
  saved: 'Сохранённые / удалённые',
  create: 'Создать виртуальное устройство',
  remove: 'Убрать из DeckWeave…',
  restore: 'Восстановить устройство',
  forget: 'Забыть сохранённые настройки…',
  removeTitle: 'Убрать это устройство?',
  forgetTitle: 'Забыть устройство навсегда?',
  removeHint:
    'Редактор закроется, выполнение действий остановится. Профили и настройки сохранятся здесь для восстановления. Физическое устройство не сбрасывается.',
  forgetHint:
    'Все сохранённые профили, действия и настройки этого экземпляра будут удалены навсегда. Остальные устройства не изменятся. Автоматическое обнаружение этого устройства останется отключённым.',
  cancel: 'Отмена',
  confirmRemove: 'Убрать устройство',
  confirmForget: 'Забыть настройки',
  emptyActive:
    'Нет активных устройств. Восстановите сохранённое, создайте виртуальное или подключите поддерживаемое оборудование.',
  emptySaved: 'Здесь появятся убранные устройства вместе с их профилями и настройками.',
  autoDiscovery:
    'Mirabox появляется автоматически; устройства установленных драйверов добавляются через «Обнаруженное оборудование». Удалённые восстанавливаются явно.',
  retained: 'Профили и настройки сохранены',
  missing: 'Поддержка устройства недоступна в этой сборке',
  readd: 'Разрешить обнаружение',
  forgotten: 'Забытое оборудование',
  forgottenHint:
    'Настройки были стёрты. Разрешение обнаружения создаст новый экземпляр при подключении оборудования.',
  profiles: 'профилей',
  restoring: 'Физическое устройство восстановится вне сети до подключения.',
  addSupport: 'Для новых моделей установите соответствующий пакет поддержки устройств.',
};
export function lifecycleText(locale: Locale, key: keyof typeof en): string {
  return (locale === 'ru' ? ru : en)[key];
}
