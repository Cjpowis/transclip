import { MotionConfig } from "motion/react"

import { Header, Hero } from "@/components/site/hero"
import { HowItWorks } from "@/components/site/how-it-works"
import { Footer, GoodToKnow, Install } from "@/components/site/install"
import { Showcases } from "@/components/site/showcases"

export function App() {
  return (
    // "user" turns off movement for visitors who ask their device for
    // less motion; each animated scene also has a still version.
    <MotionConfig reducedMotion="user">
      <a
        href="#how"
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-full focus:bg-ink focus:px-4 focus:py-2 focus:text-paper"
      >
        Skip to how it works
      </a>
      <Header />
      <main>
        <Hero />
        <HowItWorks />
        <Showcases />
        <Install />
        <GoodToKnow />
      </main>
      <Footer />
    </MotionConfig>
  )
}

export default App
