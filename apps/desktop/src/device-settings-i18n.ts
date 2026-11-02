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
  unchanged: 'Текущее значение неизвестно',
  offline: 'Сохранено. Будет отправлено при подключении.',
  pending: 'Отправляется…',
  sent_no_readback: 'Отправлено · без обратного чтения',
  error: 'Не удалось отправить',
  hint: 'Устройство не сообщает текущие значения. Яркость отправляется после отпускания ползунка.',
};
const en: Record<keyof typeof ru, string> = {
  title: 'Displays and feedback',
  screen: 'Screen brightness',
  dials: 'Dial backlight brightness',
  vibration: 'Press vibration',
  on: 'On',
  off: 'Off',
  unknown: 'Not set',
  unchanged: 'Current device value unknown',
  offline: 'Saved. Will be sent when connected.',
  pending: 'Sending…',
  sent_no_readback: 'Sent · no readback',
  error: 'Could not send',
  hint: 'The device does not report current values. Brightness is sent when the slider is released.',
};
export function deviceSettingsText(locale: Locale, key: keyof typeof ru): string {
  return (locale === 'ru' ? ru : en)[key];
}
