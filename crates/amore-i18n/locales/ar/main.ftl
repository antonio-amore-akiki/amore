# ar/main.ftl — Arabic translations for Amore.
# machine-seeded 2026-05-28.
# Antonio (native Lebanese Arabic speaker) will hand-refine these strings.
#
# BiDi convention: Unicode FSI (U+2068) / PDI (U+2069) isolation marks are applied
# around interpolated variable references per the Fluent BiDi specification.
# This ensures correct bidirectional rendering in terminals and the Inno installer.
#
# RTL note: the Inno installer and CLI terminal render full RTL via the OS BiDi engine.
# The egui GUI renders Arabic text correctly but button layout stays LTR until
# egui ships native RTL support (github.com/emilk/egui/issues/1016).

## Application identity
# machine-seeded
app-name = أمور
# machine-seeded
app-tagline = ذاكرة محلية دائمة لجميع أدوات الذكاء الاصطناعي
# machine-seeded
app-version = الإصدار ⁨{ $version }⁩

## Wizard — navigation
# machine-seeded
wizard-next = التالي
# machine-seeded
wizard-back = السابق
# machine-seeded
wizard-finish = إنهاء
# machine-seeded
wizard-cancel = إلغاء

## Wizard — screens
# machine-seeded
wizard-welcome = مرحباً بك في أمور — العمود الفقري لذاكرة الذكاء الاصطناعي المحلية
# machine-seeded
wizard-welcome-subtitle = محلي، خاص، ومجاني. لا يتطلب سحابة.
# machine-seeded
wizard-step-1 = جارٍ الاتصال بأدوات الذكاء الاصطناعي…
# machine-seeded
wizard-step-2 = اختيار موقع الذاكرة
# machine-seeded
wizard-step-3 = تثبيت المكونات المرفقة
# machine-seeded
wizard-step-4 = ربط بيئات التطوير تلقائياً
# machine-seeded
wizard-step-5 = اكتمل تقريباً!

## Installation outcomes
# machine-seeded
install-success = تم تثبيت أمور بنجاح
# machine-seeded
install-error = فشل التثبيت: ⁨{ $reason }⁩
# machine-seeded
install-progress = جارٍ التثبيت… ⁨{ $percent }⁩%
# machine-seeded
install-components = جارٍ تثبيت المكونات المرفقة…
# machine-seeded
install-ollama-wait = تم تثبيت Ollama لكنه لم يبدأ خلال 60 ثانية. حاول فتح Ollama من قائمة ابدأ.

## Uninstall
# machine-seeded
uninstall-confirm = هل أنت متأكد من رغبتك في حذف جميع بيانات أمور؟
# machine-seeded
uninstall-success = تمت إزالة تثبيت أمور بنجاح
# machine-seeded
uninstall-in-progress = جارٍ إزالة أمور…

## Memory operations
# machine-seeded
observe-success = تم حفظ الذاكرة
# machine-seeded
observe-error = فشل حفظ الذاكرة: ⁨{ $reason }⁩
# machine-seeded
recall-empty = لم يتم العثور على ذكريات تطابق استعلامك
# machine-seeded
recall-results = { $count } { $count ->
    [one] ذاكرة واحدة
    [two] ذاكرتان
   *[other] ذكريات
 }
# machine-seeded
recall-query-placeholder = ابحث في ذكرياتك…

## Tray menu
# machine-seeded
tray-tooltip = أمور — ذاكرة الذكاء الاصطناعي المحلية
# machine-seeded
tray-open-dashboard = فتح لوحة التحكم
# machine-seeded
tray-pause = إيقاف مؤقت
# machine-seeded
tray-resume = استئناف
# machine-seeded
tray-recent-activity = النشاط الأخير
# machine-seeded
tray-check-updates = التحقق من التحديثات
# machine-seeded
tray-quit = إنهاء

## Status / health
# machine-seeded
status-healthy = أمور يعمل
# machine-seeded
status-degraded = أمور يعمل في وضع محدود
# machine-seeded
status-offline = أمور غير متصل — تحقق من Ollama وQdrant
# machine-seeded
doctor-ok = جميع الأنظمة تعمل
# machine-seeded
doctor-fail = فشل فحص الصحة: ⁨{ $component }⁩ غير متاح

## Error messages
# machine-seeded
error-network = خطأ في الشبكة: ⁨{ $reason }⁩
# machine-seeded
error-disk-full = القرص ممتلئ — يلزم 500 ميغابايت على الأقل
# machine-seeded
error-permission-denied = تم رفض الإذن: ⁨{ $path }⁩
# machine-seeded
error-unknown = حدث خطأ غير متوقع
