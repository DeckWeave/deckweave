import type { Locale } from './types';

export interface SettingStatus<T> {
  requested: T | null;
  sent: T | null;
  state: 'unchanged' | 'offline' | 'pending' | 'sent_no_readback' | 'error';
  error: string | null;
}
export interface N4SettingsStatus {
  screen_brightness: SettingStatus<number>;
  dial_brightness: SettingStatus<number>;
  vibration: SettingStatus<boolean>;
}

const ru = {
  title: 'Экраны и отклик',
  screen: 'Яркость экранов',
  dials: 'Подсветка крутилок',
  vibration: 'Вибрация при нажатии',
  on: 'Включена',
  off: 'Выключена',
  unknown: 'Не задано',
  unchanged: 'Текущее значение устройства неизвестно. Изменение здесь отправит новое значение.',
  offline: 'Сохранено. Будет отправлено при подключении.',
  pending: 'Отправляется…',
  sent_no_readback: 'Отправлено устройству · обратное чтение недоступно',
  error: 'Не удалось отправить',
  hint: 'Изменения сохраняются сразу. Яркость отправляется после завершения перетаскивания. Устройство не сообщает фактические значения этих настроек.',
};
const en: Record<keyof typeof ru, string> = {
  title: 'Displays and feedback',
  screen: 'Screen brightness',
  dials: 'Dial backlight brightness',
  vibration: 'Press vibration',
  on: 'On',
  off: 'Off',
  unknown: 'Not set',
  unchanged: 'The current device value is unknown. Changing this control sends a new value.',
  offline: 'Saved. Will be sent when connected.',
  pending: 'Sending…',
  sent_no_readback: 'Sent to device · readback unavailable',
  error: 'Could not send',
  hint: 'Changes save immediately. Brightness is sent when dragging ends. The device does not report the actual values of these settings.',
};
export function deviceSettingsText(locale: Locale, key: keyof typeof ru): string {
  return (locale === 'ru' ? ru : en)[key];
}
