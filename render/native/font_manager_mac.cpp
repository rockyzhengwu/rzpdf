// Keep CoreText's byte loader without enumerating every installed font family.
#include "include/core/SkFontMgr.h"
#include "include/ports/SkFontMgr_mac_ct.h"
#include <CoreFoundation/CoreFoundation.h>
#include <CoreText/CoreText.h>

namespace {
template <typename T> class OwnedCF {
public:
    explicit OwnedCF(T value) : value_(value) {}
    ~OwnedCF() { if (value_) CFRelease(value_); }
    OwnedCF(const OwnedCF&) = delete;
    OwnedCF& operator=(const OwnedCF&) = delete;
    T get() const { return value_; }
private:
    T value_;
};
}

extern "C" SkFontMgr* rzpdf_coretext_memory_font_manager() {
    // Skia's CoreText factory dereferences the matching-descriptor array, so
    // an empty collection (which CoreText represents as nullptr) is unsafe.
    // A resolved UI-font descriptor gives it a bounded, nonempty collection.
    // This manager is used only for makeFromData, never font selection.
    OwnedCF<CTFontRef> font(CTFontCreateUIFontForLanguage(kCTFontUIFontSystem, 0, nullptr));
    if (!font.get()) return nullptr;
    OwnedCF<CTFontDescriptorRef> descriptor(CTFontCopyFontDescriptor(font.get()));
    if (!descriptor.get()) return nullptr;
    const void* values[] = {descriptor.get()};
    OwnedCF<CFArrayRef> descriptors(CFArrayCreate(nullptr, values, 1, &kCFTypeArrayCallBacks));
    if (!descriptors.get()) return nullptr;
    OwnedCF<CTFontCollectionRef> collection(
        CTFontCollectionCreateWithFontDescriptors(descriptors.get(), nullptr));
    if (!collection.get()) return nullptr;
    OwnedCF<CFArrayRef> matching(CTFontCollectionCreateMatchingFontDescriptors(collection.get()));
    if (!matching.get() || CFArrayGetCount(matching.get()) != 1) return nullptr;
    return SkFontMgr_New_CoreText(collection.get()).release();
}
